
// ====================================================================================
// src/handler.rs - 核心业务逻辑处理器
// ====================================================================================
use crate::{
    db,
    models::{ControlMessage, DbWriteCommand, InternalMessage, RoomDetailsResponse, RoomState, RoomStats, StatsQuery, WsMessage},
    state::AppState,
};
use futures_util::{SinkExt, StreamExt};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use uuid::Uuid;
use axum::extract::ws::WebSocket;

struct ConnectionInfo {
    sender: mpsc::Sender<WsMessage>,
    join_time: Instant,
    is_admin: bool,
    nickname: String,
}

// RAII Guard for connection counting
struct ConnectionGuard {
    count: Arc<AtomicU32>,
}

impl ConnectionGuard {
    fn new(count: Arc<AtomicU32>) -> Self {
        count.fetch_add(1, Ordering::Relaxed);
        Self { count }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::Relaxed);
    }
}

// 启动一个房间的中央处理器
pub async fn start_room_handler(room_id: Uuid, state: Arc<AppState>) {
    let (high_prio_tx, high_prio_rx) = mpsc::channel(100);
    let (normal_prio_tx, normal_prio_rx) = mpsc::channel(100);
    let (db_writer_tx, db_writer_rx) = mpsc::channel(1024);
    let (control_tx, control_rx) = mpsc::channel(32);
    let (stats_tx, stats_rx) = mpsc::channel(32);

    db::spawn_db_writer(state.db_pool.clone(), db_writer_rx);

    let room_state = RoomState {
        high_prio_tx,
        normal_prio_tx,
        db_writer_tx: db_writer_tx.clone(),
        control_tx,
        stats_tx,
        user_last_message_time: HashMap::new(),
        pending_join_notify: false,
        join_notify_timer_active: false,
    };

    state.rooms.lock().await.insert(room_id, room_state);

    tokio::spawn(room_message_loop(
        room_id,
        state,
        high_prio_rx,
        normal_prio_rx,
        control_rx,
        stats_rx,
    ));
}

// 处理单个WebSocket连接
pub async fn handle_socket(socket: WebSocket, state: Arc<AppState>, room_id: Uuid, user_id: String, nickname: String) {
    let _conn_guard = ConnectionGuard::new(state.total_connections.clone());
    let conn_id = Uuid::new_v4();
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let room_tx = {
        let rooms = state.rooms.lock().await;
        if let Some(room) = rooms.get(&room_id) {
            Some(room.normal_prio_tx.clone())
        } else {
            let error_msg = serde_json::to_string(&WsMessage::Error { message: "房间已关闭".to_string() }).unwrap();
            let _ = ws_sender.send(axum::extract::ws::Message::Text(error_msg)).await;
            return;
        }
    };
    let Some(room_tx) = room_tx else { return; };

    let (tx, mut rx) = mpsc::channel(10);

    if room_tx.send(InternalMessage {
        conn_id, user_id: user_id.clone(), nickname: nickname.clone(), room_id,
        content: WsMessage::UserJoined { user_id: user_id.clone(), nickname: nickname.clone() },
        sender: Some(tx.clone()),
    }).await.is_err() {
        tracing::warn!("Failed to register new user, room handler might be down.");
        return;
    }

    let ws_sender = Arc::new(tokio::sync::Mutex::new(ws_sender));

    // tokio::spawn 发送消息部分也要用 Arc clone
    let ws_sender_clone = ws_sender.clone();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let text_msg = serde_json::to_string(&msg).unwrap();
            let mut sender = ws_sender_clone.lock().await;
            if sender.send(axum::extract::ws::Message::Text(text_msg)).await.is_err() { break; }
        }
    });

    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Ok(ws_msg) = WsMessage::try_from(msg) {
            match ws_msg {
                WsMessage::Ping { timestamp } => {
                    let pong = WsMessage::Pong { timestamp };
                    let text_msg = serde_json::to_string(&pong).unwrap();
                    let mut sender = ws_sender.lock().await;
                    let _ = sender.send(axum::extract::ws::Message::Text(text_msg)).await;
                },
                _ => {
                    if room_tx.send(InternalMessage {
                        conn_id, user_id: user_id.clone(), nickname: nickname.clone(), room_id,
                        content: ws_msg, sender: None,
                    }).await.is_err() { break; }
                }
            }
        }
    }

    let _ = room_tx.send(InternalMessage {
        conn_id, user_id: user_id.clone(), nickname: nickname.clone(), room_id,
        content: WsMessage::UserLeft { user_id: user_id.clone(), nickname: nickname.clone() }, sender: None,
    }).await;
}

// 房间的中央消息处理循环
pub async fn room_message_loop(
    room_id: Uuid,
    state: Arc<AppState>,
    mut high_prio_rx: mpsc::Receiver<InternalMessage>,
    mut normal_prio_rx: mpsc::Receiver<InternalMessage>,
    mut control_rx: mpsc::Receiver<ControlMessage>,
    mut stats_rx: mpsc::Receiver<StatsQuery>,
) {
    let mut connections: HashMap<Uuid, ConnectionInfo> = HashMap::new();
    let mut user_id_to_conn_id: HashMap<String, Uuid> = HashMap::new();
    let mut muted_users: HashSet<String> = HashSet::new();
    let mut admin_users = db::load_admins_for_room(&state.db_pool, room_id).await.unwrap_or_default();
    let mut banned_users = db::load_bans_for_room(&state.db_pool, room_id).await.unwrap_or_default();
    let mut stats = RoomStats::default();
    let start_time = chrono::Utc::now().timestamp();

    const LOW_PRIO_SLICE: usize = 200; // 每处理200条低优先级消息让步一次

    loop {
        // 优先处理高优先级消息
        tokio::select! {
            Some(msg) = high_prio_rx.recv() => {
                handle_message(msg, &mut connections, &mut user_id_to_conn_id, &mut muted_users, &mut banned_users, &admin_users, &state, &mut stats).await;
            },
            // 其它高优先级通道
            Some(ctrl_msg) = control_rx.recv() => {
                match ctrl_msg {
                    ControlMessage::ResetAdmins(new_admins) => {
                        admin_users = new_admins;
                        tracing::info!("Admins for room {} have been reset.", room_id);
                    }
                    ControlMessage::UnbanUser(user_id) => {
                        if banned_users.remove(&user_id) {
                            tracing::info!("User {} has been unbanned from room {}.", user_id, room_id);
                        }
                    }
                }
            },
            Some(query) = stats_rx.recv() => {
                // 查询房间名称
                let room_name = match db::get_room_basic_info(&state.db_pool, room_id).await {
                    Ok(Some(info)) => info.room_name,
                    _ => "".to_string(),
                };
                let response = RoomDetailsResponse {
                    room_id,
                    room_name,
                    admin_user_ids: admin_users.clone(),
                    start_time,
                    stats: stats.clone(),
                };
                let _ = query.response_tx.send(response);
            },
            else => {
                // 分片处理部分修正如下：
                let mut count = 0;
                loop {
                    // 先检查高优先级队列是否有消息
                    match high_prio_rx.try_recv() {
                        Ok(msg) => {
                            handle_message(msg, &mut connections, &mut user_id_to_conn_id, &mut muted_users, &mut banned_users, &admin_users, &state, &mut stats).await;
                            break; // 让出分片，回到tokio::select!
                        },
                        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {},
                        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
                    }
                    // 处理一条低优先级消息
                    match normal_prio_rx.try_recv() {
                        Ok(msg) => {
                            handle_message(msg, &mut connections, &mut user_id_to_conn_id, &mut muted_users, &mut banned_users, &admin_users, &state, &mut stats).await;
                            count += 1;
                            if count >= LOW_PRIO_SLICE {
                                // 分片让步，回到tokio::select!，优先响应高优先级
                                break;
                            }
                        },
                        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => { break; },
                        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => { break; },
                    }
                }
                // 如果有pending_join_notify为true，则广播一次当前房间人数，并重置pending_join_notify
                if let Some(room_state) = state.rooms.lock().await.get_mut(&room_id) {
                    if room_state.pending_join_notify {
                        broadcast(&connections, WsMessage::RoomStats { current_users: stats.current_users, peak_users: stats.peak_users }, None).await;
                        room_state.pending_join_notify = false;
                    }
                }
                // 如果房间即将关闭，广播当前房间人数
                if connections.is_empty() {
                    tracing::info!("Room {} handler shutting down.", room_id);
                    broadcast(&connections, WsMessage::RoomStats { current_users: stats.current_users, peak_users: stats.peak_users }, None).await;
                    break;
                }
            }
        }
    }
}

// 具体的消息处理逻辑
async fn handle_message(
    msg: InternalMessage,
    connections: &mut HashMap<Uuid, ConnectionInfo>,
    user_id_to_conn_id: &mut HashMap<String, Uuid>,
    muted_users: &mut HashSet<String>,
    banned_users: &mut HashSet<String>,
    admin_users: &HashSet<String>,
    state: &Arc<AppState>,
    stats: &mut RoomStats,
) {
    let db_writer_tx = {
        let rooms = state.rooms.lock().await;
        rooms.get(&msg.room_id).map(|r| r.db_writer_tx.clone())
    };
    let Some(db_writer_tx) = db_writer_tx else { return; };

    match msg.content {
        WsMessage::UserJoined { user_id: _, nickname: _ } => {
            let sender = msg.sender.expect("UserJoined message must have a sender");

            if banned_users.contains(&msg.user_id) {
                let _ = sender.send(WsMessage::Error { message: "你已被踢出该房间，无法再次加入".to_string() }).await;
                return;
            }

            if let Some(old_conn_id) = user_id_to_conn_id.remove(&msg.user_id) {
                if let Some(old_conn) = connections.remove(&old_conn_id) {
                    stats.current_users = stats.current_users.saturating_sub(1);
                    let _ = old_conn.sender.send(WsMessage::YouAreKicked).await;
                }
            }
            
            let is_admin = admin_users.contains(&msg.user_id);
            let is_muted = muted_users.contains(&msg.user_id);
            let welcome_msg = WsMessage::WelcomeInfo { user_id: msg.user_id.clone(), nickname: msg.nickname.clone(), is_muted };
            if sender.send(welcome_msg).await.is_err() { return; }

            let conn_info = ConnectionInfo { sender, join_time: Instant::now(), is_admin, nickname: msg.nickname.clone() };
            connections.insert(msg.conn_id, conn_info);
            user_id_to_conn_id.insert(msg.user_id.clone(), msg.conn_id);

            stats.current_users += 1;
            stats.total_joins += 1;
            if stats.current_users > stats.peak_users {
                stats.peak_users = stats.current_users;
            }

            let _ = db_writer_tx.send(DbWriteCommand::UserJoined { user_id: msg.user_id.clone(), nickname: msg.nickname.clone(), room_id: msg.room_id }).await;
            // --- 新增：用户加入通知截流 ---
            {
                let mut rooms = state.rooms.lock().await;
                if let Some(room_state) = rooms.get_mut(&msg.room_id) {
                    room_state.pending_join_notify = true;
                    if !room_state.join_notify_timer_active {
                        room_state.join_notify_timer_active = true;
                        let state_clone = state.clone();
                        let room_id_clone = msg.room_id;
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            let mut rooms = state_clone.rooms.lock().await;
                            if let Some(room_state) = rooms.get_mut(&room_id_clone) {
                                if room_state.pending_join_notify {
                                    // 获取当前房间人数
                                    let current_users = room_state.stats_tx.clone(); // 这里实际应从内存connections或stats获取
                                    // 这里直接广播当前房间人数
                                    // 需要访问connections和stats，建议在主循环else分支处理
                                    // 这里只重置标志
                                    room_state.pending_join_notify = false;
                                }
                                room_state.join_notify_timer_active = false;
                            }
                        });
                    }
                }
            }
            // --- 截流逻辑结束 ---
        }
        WsMessage::UserLeft { user_id: _, nickname: _ } => {
            if let Some(conn_info) = connections.remove(&msg.conn_id) {
                user_id_to_conn_id.remove(&msg.user_id);
                stats.current_users = stats.current_users.saturating_sub(1);
                let _ = db_writer_tx.send(DbWriteCommand::UserLeft { user_id: msg.user_id.clone(), nickname: msg.nickname.clone(), room_id: msg.room_id, join_time: conn_info.join_time }).await;
            }
        }
        WsMessage::SendMessage { ref content } => {
            let conn_info = if let Some(info) = connections.get(&msg.conn_id) { info } else { return; };
            if muted_users.contains(&msg.user_id) {
                let _ = conn_info.sender.send(WsMessage::YouAreMuted).await;
                return;
            }
            let is_admin = conn_info.is_admin;
            if !is_admin {
                // 频率限制
                let rooms = state.rooms.lock().await;
                if let Some(room_state) = rooms.get(&msg.room_id) {
                    let now = chrono::Utc::now().timestamp();
                    let last = room_state.user_last_message_time.get(&msg.user_id).copied().unwrap_or(0);
                    let min_interval = state.config.user_message_interval_secs as i64;
                    if now - last < min_interval {
                        let _ = conn_info.sender.send(WsMessage::Error { message: format!("发送过于频繁，请{}秒后再试", min_interval - (now - last)) }).await;
                        return;
                    }
                }
            }
            // 更新发言时间
            {
                let mut rooms = state.rooms.lock().await;
                if let Some(room_state) = rooms.get_mut(&msg.room_id) {
                    room_state.user_last_message_time.insert(msg.user_id.clone(), chrono::Utc::now().timestamp());
                }
            }
            let _ = db_writer_tx.send(DbWriteCommand::ChatMessage { user_id: msg.user_id.clone(), nickname: msg.nickname.clone(), room_id: msg.room_id, content: content.clone() }).await;
            broadcast(connections, WsMessage::Message { from: msg.user_id, nickname: msg.nickname, content: content.clone(), is_admin }, None).await;
        }
        WsMessage::KickUser { user_id } => {
            let conn_info = if let Some(info) = connections.get(&msg.conn_id) { info } else { return; };
            if !conn_info.is_admin { return; }
            
            banned_users.insert(user_id.clone());
            let _ = db_writer_tx.send(DbWriteCommand::BanUser { user_id: user_id.clone(), room_id: msg.room_id }).await;

            if let Some(target_conn_id) = user_id_to_conn_id.remove(&user_id) {
                if let Some(target_conn) = connections.remove(&target_conn_id) {
                    stats.current_users = stats.current_users.saturating_sub(1);
                    let _ = target_conn.sender.send(WsMessage::YouAreKicked).await;
                }
            }
        }
        WsMessage::MuteUser { user_id } => {
            let conn_info = if let Some(info) = connections.get(&msg.conn_id) { info } else { return; };
            if !conn_info.is_admin { return; }
            muted_users.insert(user_id.clone());
        }
        WsMessage::CustomEvent { ref event_type, ref payload } => {
            let conn_info = if let Some(info) = connections.get(&msg.conn_id) { info } else { return; };
            if !conn_info.is_admin { return; } // 只有管理员可发
            broadcast(connections, WsMessage::CustomEvent { event_type: event_type.clone(), payload: payload.clone() }, None).await;
        }
        _ => {}
    }
}

// 广播消息给房间内的所有用户
async fn broadcast(connections: &HashMap<Uuid, ConnectionInfo>, msg: WsMessage, exclude_conn_id: Option<Uuid>) {
    for (conn_id, conn_info) in connections.iter() {
        if Some(*conn_id) == exclude_conn_id { continue; }
        if conn_info.sender.send(msg.clone()).await.is_err() {}
    }
}



impl TryFrom<axum::extract::ws::Message> for WsMessage {
    type Error = serde_json::Error;
    fn try_from(msg: axum::extract::ws::Message) -> Result<Self, <WsMessage as TryFrom<axum::extract::ws::Message>>::Error> {
        if let axum::extract::ws::Message::Text(text) = msg {
            serde_json::from_str(&text)
        } else {
            Err(serde_json::from_str::<WsMessage>("").unwrap_err())
        }
    }
}
