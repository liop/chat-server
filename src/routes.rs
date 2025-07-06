
// ====================================================================================
// src/routes.rs - HTTP路由处理
// ====================================================================================
use crate::{
    config::Config,
    db,
    error::AppError,
    handler,
    models::{CreateRoomRequest, CreateRoomResponse, DataSyncPayload, ResetAdminsRequest, RoomDetailsResponse, StatsQuery, ControlMessage, RoomBasicInfo, ChatHistoryPage, SessionHistoryPage, PaginationQuery},
    state::AppState,
    sync::SyncService,
    callback::CallbackService,
};
use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use headers::{HeaderMap, HeaderValue};
use serde::Deserialize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use uuid::Uuid;

// 健康检查
pub async fn health_check() -> impl IntoResponse {
    "OK"
}

// WebSocket处理器
#[derive(Deserialize)]
pub struct WsConnectQuery {
    user_id: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(room_id): Path<Uuid>,
    Query(query): Query<WsConnectQuery>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // 负载保护检查
    if state.total_connections.load(Ordering::Relaxed) >= state.config.max_connections {
        return AppError::ServiceUnavailable("服务器连接数已达上限".to_string()).into_response();
    }
    
    // 检查房间是否存在
    if !state.rooms.lock().await.contains_key(&room_id) {
        return AppError::NotFound(format!("Room {} not found", room_id)).into_response();
    }

    // 升级连接
    ws.on_upgrade(move |socket| async move {
        handler::handle_socket(socket, state, room_id, query.user_id).await
    })
}

// 创建房间
pub async fn create_room(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<CreateRoomRequest>,
) -> Result<Json<CreateRoomResponse>, AppError> {
    check_auth(&headers, &state.config)?;

    let room_id = Uuid::new_v4();
    let room_name = payload.room_name;
    let admin_ids = payload.admin_user_ids;

    let mut tx = state.db_pool.begin().await?;
    sqlx::query("INSERT INTO rooms (id, name, created_at) VALUES (?, ?, ?)")
        .bind(room_id.to_string()).bind(&room_name).bind(chrono::Utc::now().timestamp())
        .execute(&mut *tx).await?;

    for admin_id in &admin_ids {
        sqlx::query("INSERT INTO room_admins (room_id, user_id) VALUES (?, ?)")
            .bind(room_id.to_string()).bind(admin_id)
            .execute(&mut *tx).await?;
    }
    tx.commit().await?;

    handler::start_room_handler(room_id, state.clone()).await;

    // 创建房间后立即同步数据
    if let Some(_callback_url) = &state.config.data_callback_url {
        let state_clone = state.clone();
        tokio::spawn(async move {
            if let Err(e) = SyncService::sync_room(room_id, &state_clone, &state_clone.config).await {
                tracing::error!("创建房间后同步失败: {}", e);
            } else {
                tracing::info!("创建房间后同步成功，房间ID: {}", room_id);
            }
        });
    }

    // 发送房间创建事件回调
    let callback_service = CallbackService::new(state.config.clone());
    tokio::spawn(async move {
        callback_service.send_room_created(room_id, room_name, admin_ids).await;
    });

    let websocket_url = format!("/ws/rooms/{}", room_id);
    Ok(Json(CreateRoomResponse { room_id, websocket_url }))
}

// 查询所有房间
pub async fn list_rooms(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<RoomDetailsResponse>>, AppError> {
    check_auth(&headers, &state.config)?;

    let rooms_guard = state.rooms.lock().await;
    let mut details_list = Vec::new();

    for room_state in rooms_guard.values() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if room_state.stats_tx.send(StatsQuery { response_tx: tx }).await.is_ok() {
            if let Ok(details) = rx.await {
                details_list.push(details);
            }
        }
    }

    Ok(Json(details_list))
}

// 获取房间基础信息（拆分后的接口）
pub async fn get_rooms_basic_info(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<RoomBasicInfo>>, AppError> {
    check_auth(&headers, &state.config)?;

    let mut rooms_info = Vec::new();
    
    // 收集所有房间信息，避免生命周期问题
    let room_info: Vec<(Uuid, tokio::sync::mpsc::Sender<StatsQuery>)> = {
        let rooms = state.rooms.lock().await;
        rooms.iter()
            .map(|(room_id, room)| (*room_id, room.stats_tx.clone()))
            .collect()
    };

    for (room_id, stats_tx) in room_info {
        // 获取数据库中的房间基础信息
        if let Ok(Some(mut room_basic)) = db::get_room_basic_info(&state.db_pool, room_id).await {
            // 获取当前连接数
            let (tx, rx) = tokio::sync::oneshot::channel();
            if stats_tx.send(StatsQuery { response_tx: tx }).await.is_ok() {
                if let Ok(details) = rx.await {
                    room_basic.current_connections = details.stats.current_users;
                }
            }
            rooms_info.push(room_basic);
        }
    }

    Ok(Json(rooms_info))
}

// 获取聊天记录（分页）
pub async fn get_chat_history(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<ChatHistoryPage>, AppError> {
    check_auth(&headers, &state.config)?;

    // 检查房间是否存在
    if !state.rooms.lock().await.contains_key(&room_id) {
        return Err(AppError::NotFound(format!("Room {} not found", room_id)));
    }

    let chat_page = db::get_chat_history_page(&state.db_pool, room_id, &query).await?;
    Ok(Json(chat_page))
}

// 获取会话历史（分页）
pub async fn get_session_history(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<SessionHistoryPage>, AppError> {
    check_auth(&headers, &state.config)?;

    // 检查房间是否存在
    if !state.rooms.lock().await.contains_key(&room_id) {
        return Err(AppError::NotFound(format!("Room {} not found", room_id)));
    }

    let session_page = db::get_session_history_page(&state.db_pool, room_id, &query).await?;
    Ok(Json(session_page))
}

// 重置管理员
pub async fn reset_admins(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    Json(payload): Json<ResetAdminsRequest>,
) -> Result<StatusCode, AppError> {
    check_auth(&headers, &state.config)?;

    let rooms = state.rooms.lock().await;
    if let Some(room) = rooms.get(&room_id) {
        // 更新数据库
        let mut tx = state.db_pool.begin().await?;
        sqlx::query("DELETE FROM room_admins WHERE room_id = ?").bind(room_id.to_string()).execute(&mut *tx).await?;
        for admin_id in &payload.admin_user_ids {
            sqlx::query("INSERT INTO room_admins (room_id, user_id) VALUES (?, ?)")
                .bind(room_id.to_string()).bind(admin_id)
                .execute(&mut *tx).await?;
        }
        tx.commit().await?;

        // 发送控制消息通知房间处理器更新内存状态
        room.control_tx.send(ControlMessage::ResetAdmins(payload.admin_user_ids)).await
            .map_err(|_| AppError::InternalServerError("房间控制消息发送失败".to_string()))?;
        Ok(StatusCode::OK)
    } else {
        Err(AppError::NotFound(format!("Room {} not found", room_id)))
    }
}

// 解除用户封禁
pub async fn unban_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((room_id, user_id)): Path<(Uuid, String)>,
) -> Result<StatusCode, AppError> {
    check_auth(&headers, &state.config)?;

    let rooms = state.rooms.lock().await;
    if let Some(room) = rooms.get(&room_id) {
        // 更新数据库
        sqlx::query("DELETE FROM room_bans WHERE room_id = ? AND user_id = ?")
            .bind(room_id.to_string()).bind(&user_id)
            .execute(&state.db_pool).await?;

        // 发送控制消息
        room.control_tx.send(ControlMessage::UnbanUser(user_id)).await
            .map_err(|_| AppError::InternalServerError("房间控制消息发送失败".to_string()))?;
        Ok(StatusCode::OK)
    } else {
        Err(AppError::NotFound(format!("Room {} not found", room_id)))
    }
}

// 关闭房间
pub async fn close_room(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    check_auth(&headers, &state.config)?;

    let room_state = {
        let mut rooms = state.rooms.lock().await;
        rooms.remove(&room_id)
    };

    if let Some(room) = room_state {
        // 获取房间统计信息用于回调
        let final_stats = {
            let (tx, rx) = tokio::sync::oneshot::channel();
            if room.stats_tx.send(StatsQuery { response_tx: tx }).await.is_ok() {
                rx.await.ok().map(|details| details.stats)
            } else {
                None
            }
        };

        // 触发数据同步
        if let Some(callback_url) = &state.config.data_callback_url {
            let (tx, rx) = tokio::sync::oneshot::channel();
            if room.stats_tx.send(StatsQuery { response_tx: tx }).await.is_ok() {
                if let Ok(details) = rx.await {
                     let sync_data = db::get_data_for_sync(&state.db_pool, room_id, details).await?;
                     tokio::spawn(send_data_sync(callback_url.clone(), sync_data));
                }
            }
        }

        // 发送房间关闭事件回调
        if let Some(stats) = final_stats {
            let callback_service = CallbackService::new(state.config.clone());
            tokio::spawn(async move {
                callback_service.send_room_closed(room_id, stats).await;
            });
        }

        drop(room);
    } else {
        return Err(AppError::NotFound(format!("Room {} not found", room_id)));
    }

    Ok(StatusCode::NO_CONTENT)
}

// 获取同步数据（供外部系统主动拉取）
pub async fn get_sync_data(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<DataSyncPayload>>, AppError> {
    check_auth(&headers, &state.config)?;

    let sync_data = SyncService::get_all_sync_data(&state)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(sync_data))
}

// 手动触发同步
pub async fn trigger_sync(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    check_auth(&headers, &state.config)?;

    tokio::spawn(async move {
        if let Err(e) = SyncService::trigger_manual_sync(&state, &state.config).await {
            tracing::error!("手动触发同步失败: {}", e);
        } else {
            tracing::info!("手动触发同步成功");
        }
    });

    Ok(StatusCode::ACCEPTED)
}

// 认证辅助函数
fn check_auth(headers: &HeaderMap, config: &Config) -> Result<(), AppError> {
    if let Some(key) = headers.get("X-Api-Key") {
        if key == &HeaderValue::from_str(&config.admin_api_key).unwrap() {
            return Ok(());
        }
    }
    Err(AppError::Unauthorized)
}

// 异步发送数据同步
async fn send_data_sync(url: String, payload: DataSyncPayload) {
    let client = reqwest::Client::new();
    if let Err(e) = client.post(&url).json(&payload).send().await {
        tracing::error!("Failed to send data sync to {}: {}", url, e);
    } else {
        tracing::info!("Successfully sent data sync for room {}", payload.room_id);
    }
}
 