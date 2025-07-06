/*
 * ====================================================================================
 * README.md - 项目说明
 * ====================================================================================
 *
 * 这是一个使用Rust编写的高性能、按需启动的匿名聊天室后端服务。
 *
 * ## 功能特性
 * - 基于Axum和Tokio，纯异步实现，支持大规模并发连接。
 * - 使用SQLite作为嵌入式数据库（推荐内存模式），无需外部依赖，易于部署。
 * - 提供完整的HTTP管理API，用于被外部服务控制（创建/查询/重置管理员/解禁/关闭房间）。
 * - 支持多房间、多管理员、踢人、禁言等功能。
 * - 支持通过环境变量设置最大连接数，当达到上限时拒绝新连接，实现负载保护。
 * - 权限校验（管理员、封禁）在内存中完成，移除了连接时的数据库查询，以应对海量用户瞬时涌入。
 * - 踢出用户后，该用户将被永久禁止加入该房间，但可通过API解禁。
 * - 内置管理员消息高优先级处理机制。
 * - 采用异步批处理优化数据库写入，应对高频用户进出。
 * - 支持在房间关闭后，将包含统计信息的完整数据同步到外部回调URL。
 *
 * ## 如何运行
 *
 * 1. **安装 Rust**: 如果你还没有安装，请访问 https://www.rust-lang.org/
 *
 * 2. **创建项目**:
 * ```bash
 * cargo new chat_server
 * cd chat_server
 * ```
 *
 * 3. **配置 `Cargo.toml`**:
 * 将下面的 `Cargo.toml` 内容复制到你的 `Cargo.toml` 文件中。
 *
 * 4. **复制代码**:
 * 将下面的所有 `src/*.rs` 文件内容复制到你的 `src` 目录下对应的文件中。
 *
 * 5. **设置环境变量**:
 * 在运行前，需要设置以下环境变量：
 * ```bash
 * # 服务监听地址
 * export BIND_ADDRESS="0.0.0.0:3000"
 * # 推荐使用内存数据库以获得最佳性能和自动清理
 * export DATABASE_URL="sqlite::memory:"
 * # 或者使用文件数据库
 * # export DATABASE_URL="sqlite:chat_server.db"
 * # 管理API的安全密钥
 * export ADMIN_API_KEY="your-super-secret-key"
 * # 设置服务器最大连接数上限
 * export MAX_CONNECTIONS=100000
 * # (可选) 房间关闭后数据同步的回调URL
 * export DATA_CALLBACK_URL="http://your-java-service/api/chat_data_sync"
 * ```
 *
 * 6. **编译并运行**:
 * ```bash
 * cargo run --release
 * ```
 *
 * ## API 使用
 *
 * - **健康检查**: `GET http://localhost:3000/management/health`
 * - **创建房间**: `POST http://localhost:3000/management/rooms`
 * - **查询所有房间**: `GET http://localhost:3000/management/rooms`
 * - **重置管理员**: `PUT http://localhost:3000/management/rooms/{room_id}/admins`
 * - **解除用户封禁**: `DELETE http://localhost:3000/management/rooms/{room_id}/bans/{user_id}`
 * - **关闭房间**: `DELETE http://localhost:3000/management/rooms/{room_id}`
 * - **加入房间 (WebSocket)**: `ws://localhost:3000/ws/rooms/{room_id}?user_id=user123`
 *
 * (所有管理API都需要在Header中提供 `X-Api-Key: your-super-secret-key`)
 *
 */

/*
 * ====================================================================================
 * Cargo.toml - 项目依赖配置
 * ====================================================================================
 */
/*
[package]
name = "chat_server"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = { version = "0.7", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
futures-util = { version = "0.3", default-features = false, features = ["sink", "std"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "uuid", "time"] }
uuid = { version = "1.8", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "1.0"
dotenvy = "0.15"
reqwest = { version = "0.12", features = ["json"] }
http = "1.1"
headers = "0.4"
tower-http = { version = "0.5.0", features = ["cors"] }
chrono = { version = "0.4", features = ["serde"] }
*/

// ====================================================================================
// src/main.rs - 应用入口
// ====================================================================================
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use sqlx::sqlite::SqlitePoolOptions;
use std::net::SocketAddr;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod db;
mod error;
mod handler;
mod models;
mod routes;
mod state;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志记录
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "chat_server=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 加载配置
    let config = Config::from_env().expect("Failed to load config");
    let db_config = config.clone();
    let app_state_config = config.clone();

    // 连接数据库并运行迁移
    let pool = SqlitePoolOptions::new()
        .max_connections(10) // 增加连接池大小以支持更多并发API请求
        .connect(&db_config.database_url)
        .await
        .expect("Failed to connect to database");
    db::migrate(&pool).await.expect("Failed to run migrations");

    // 创建共享的应用状态
    let app_state = Arc::new(AppState {
        db_pool: pool.clone(),
        rooms: Mutex::new(std::collections::HashMap::new()),
        total_connections: Arc::new(AtomicU32::new(0)),
        config: app_state_config,
    });

    // 定义CORS策略
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 创建Axum路由
    let app = Router::new()
        .route("/management/health", get(routes::health_check))
        .route("/management/rooms", get(routes::list_rooms).post(routes::create_room))
        .route(
            "/management/rooms/:room_id",
            delete(routes::close_room),
        )
        .route(
            "/management/rooms/:room_id/admins",
            put(routes::reset_admins),
        )
        .route(
            "/management/rooms/:room_id/bans/:user_id",
            delete(routes::unban_user),
        )
        .route("/ws/rooms/:room_id", get(routes::ws_handler))
        .with_state(app_state)
        .layer(cors);

    // 启动服务器
    let addr: SocketAddr = config
        .bind_address
        .parse()
        .expect("Invalid bind address");
    tracing::debug!("服务器正在监听于 {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();

    Ok(())
}


// ====================================================================================
// src/config.rs - 配置管理
// ====================================================================================
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_address: String,
    pub admin_api_key: String,
    pub data_callback_url: Option<String>,
    pub max_connections: u32,
}

impl Config {
    pub fn from_env() -> Result<Self, dotenvy::Error> {
        dotenvy::dotenv().ok();
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set"),
            bind_address: std::env::var("BIND_ADDRESS")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            admin_api_key: std::env::var("ADMIN_API_KEY")
                .expect("ADMIN_API_KEY must be set"),
            data_callback_url: std::env::var("DATA_CALLBACK_URL").ok(),
            max_connections: std::env::var("MAX_CONNECTIONS")
                .unwrap_or_else(|_| "100000".to_string())
                .parse()
                .expect("MAX_CONNECTIONS must be a valid number"),
        })
    }
}

// ====================================================================================
// src/error.rs - 自定义错误类型
// ====================================================================================
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("SQLx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Unauthorized: Invalid API Key")]
    Unauthorized,
    #[error("Not Found: {0}")]
    NotFound(String),
    #[error("Bad Request: {0}")]
    BadRequest(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Service Unavailable: {0}")]
    ServiceUnavailable(String),
    #[error("Internal Server Error")]
    InternalServerError,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Sqlx(e) => {
                tracing::error!("SQLx error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string())
            }
            AppError::Reqwest(e) => {
                tracing::error!("Reqwest error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "External service communication error".to_string())
            }
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            AppError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
            AppError::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "An internal error occurred".to_string(),
            ),
        };

        let body = Json(json!({ "error": error_message }));
        (status, body).into_response()
    }
}


// ====================================================================================
// src/models.rs - 数据模型定义
// ====================================================================================
use crate::db;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tokio::sync::mpsc;
use uuid::Uuid;

// WebSocket消息模型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    SendMessage { content: String },
    KickUser { user_id: String },
    MuteUser { user_id: String },
    Message { from: String, content: String, is_admin: bool },
    UserJoined { user_id: String },
    UserLeft { user_id: String },
    YouAreKicked,
    YouAreMuted,
    UserMuted { user_id: String },
    Error { message: String },
    System { message: String },
    WelcomeInfo { user_id: String, is_muted: bool },
}

// 房间内部状态
#[derive(Debug)]
pub struct RoomState {
    pub high_prio_tx: mpsc::Sender<InternalMessage>,
    pub normal_prio_tx: mpsc::Sender<InternalMessage>,
    pub db_writer_tx: mpsc::Sender<DbWriteCommand>,
    pub control_tx: mpsc::Sender<ControlMessage>,
    pub stats_tx: mpsc::Sender<StatsQuery>,
}

// 房间控制消息
#[derive(Debug)]
pub enum ControlMessage {
    ResetAdmins(HashSet<String>),
    UnbanUser(String),
}

// 房间统计查询
#[derive(Debug)]
pub struct StatsQuery {
    pub response_tx: tokio::sync::oneshot::Sender<RoomDetailsResponse>,
}

// 内部消息
#[derive(Debug)]
pub struct InternalMessage {
    pub conn_id: Uuid,
    pub user_id: String,
    pub room_id: Uuid,
    pub content: WsMessage,
    pub sender: Option<mpsc::Sender<WsMessage>>,
}

// 数据库写入命令
#[derive(Debug)]
pub enum DbWriteCommand {
    UserJoined { user_id: String, room_id: Uuid },
    UserLeft { user_id: String, room_id: Uuid, join_time: std::time::Instant },
    ChatMessage { user_id: String, room_id: Uuid, content: String },
    BanUser { user_id: String, room_id: Uuid },
    UnbanUser { user_id: String, room_id: Uuid },
}

// API请求/响应模型
#[derive(Deserialize)]
pub struct CreateRoomRequest {
    pub room_name: String,
    pub admin_user_ids: Vec<String>,
}

#[derive(Serialize)]
pub struct CreateRoomResponse {
    pub room_id: Uuid,
    pub websocket_url: String,
}

#[derive(Deserialize)]
pub struct ResetAdminsRequest {
    pub admin_user_ids: HashSet<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct RoomStats {
    pub current_users: u32,
    pub peak_users: u32,
    pub total_joins: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct RoomDetailsResponse {
    pub room_id: Uuid,
    pub admin_user_ids: HashSet<String>,
    pub start_time: i64,
    #[serde(flatten)]
    pub stats: RoomStats,
}

#[derive(Serialize)]
pub struct DataSyncPayload {
    pub room_id: Uuid,
    pub admin_user_ids: HashSet<String>,
    pub start_time: i64,
    #[serde(flatten)]
    pub stats: RoomStats,
    pub chat_history: Vec<db::ChatHistoryEntry>,
    pub session_history: Vec<db::SessionHistoryEntry>,
}


// ====================================================================================
// src/state.rs - 共享应用状态
// ====================================================================================
use crate::{models::RoomState, Config};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct AppState {
    pub db_pool: SqlitePool,
    pub rooms: Mutex<HashMap<Uuid, RoomState>>,
    pub total_connections: Arc<AtomicU32>,
    pub config: Config,
}

// ====================================================================================
// src/db.rs - 数据库交互
// ====================================================================================
use crate::error::AppError;
use crate::models::{DataSyncPayload, DbWriteCommand, RoomDetailsResponse, RoomStats};
use chrono::Utc;
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(FromRow, Serialize, Debug)]
pub struct ChatHistoryEntry {
    pub user_id: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(FromRow, Serialize, Debug)]
pub struct SessionHistoryEntry {
    pub user_id: String,
    pub join_time: i64,
    pub leave_time: i64,
    pub duration_seconds: i64,
}

#[derive(FromRow)]
pub struct RoomInfo {
    pub id: String,
    pub created_at: i64,
}

// 初始化数据库表
pub async fn migrate(pool: &SqlitePool) -> Result<(), AppError> {
    sqlx::query(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        CREATE TABLE IF NOT EXISTS rooms (id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS room_admins (room_id TEXT NOT NULL, user_id TEXT NOT NULL, PRIMARY KEY (room_id, user_id));
        CREATE TABLE IF NOT EXISTS chat_history (id INTEGER PRIMARY KEY AUTOINCREMENT, room_id TEXT NOT NULL, user_id TEXT NOT NULL, content TEXT NOT NULL, created_at INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS room_sessions (id INTEGER PRIMARY KEY AUTOINCREMENT, room_id TEXT NOT NULL, user_id TEXT NOT NULL, join_time INTEGER NOT NULL, leave_time INTEGER, duration_seconds INTEGER);
        CREATE TABLE IF NOT EXISTS room_bans (room_id TEXT NOT NULL, user_id TEXT NOT NULL, PRIMARY KEY (room_id, user_id));
        ",
    )
    .execute(pool)
    .await?;
    Ok(())
}

// 启动后台数据库写入器
pub fn spawn_db_writer(pool: SqlitePool, mut rx: mpsc::Receiver<DbWriteCommand>) {
    tokio::spawn(async move {
        let mut buffer = Vec::with_capacity(100);
        loop {
            match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
                Ok(Some(cmd)) => buffer.push(cmd),
                Ok(None) => break,
                Err(_) => {}
            }
            while buffer.len() < 100 {
                if let Ok(cmd) = rx.try_recv() { buffer.push(cmd); } else { break; }
            }
            if !buffer.is_empty() {
                if let Err(e) = write_batch(&pool, &buffer).await { tracing::error!("Failed to write batch to DB: {}", e); }
                buffer.clear();
            }
        }
    });
}

// 批量写入
async fn write_batch(pool: &SqlitePool, commands: &[DbWriteCommand]) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    for cmd in commands {
        match cmd {
            DbWriteCommand::UserJoined { user_id, room_id } => {
                sqlx::query("INSERT INTO room_sessions (room_id, user_id, join_time) VALUES (?, ?, ?)")
                    .bind(room_id.to_string()).bind(user_id).bind(Utc::now().timestamp())
                    .execute(&mut *tx).await?;
            }
            DbWriteCommand::UserLeft { user_id, room_id, join_time } => {
                let duration = join_time.elapsed().as_secs() as i64;
                sqlx::query("UPDATE room_sessions SET leave_time = ?, duration_seconds = ? WHERE user_id = ? AND room_id = ? AND leave_time IS NULL")
                    .bind(Utc::now().timestamp()).bind(duration).bind(user_id).bind(room_id.to_string())
                    .execute(&mut *tx).await?;
            }
            DbWriteCommand::ChatMessage { user_id, room_id, content } => {
                sqlx::query("INSERT INTO chat_history (room_id, user_id, content, created_at) VALUES (?, ?, ?, ?)")
                    .bind(room_id.to_string()).bind(user_id).bind(content).bind(Utc::now().timestamp())
                    .execute(&mut *tx).await?;
            }
            DbWriteCommand::BanUser { user_id, room_id } => {
                sqlx::query("INSERT OR IGNORE INTO room_bans (room_id, user_id) VALUES (?, ?)")
                    .bind(room_id.to_string()).bind(user_id)
                    .execute(&mut *tx).await?;
            }
            DbWriteCommand::UnbanUser { user_id, room_id } => {
                sqlx::query("DELETE FROM room_bans WHERE room_id = ? AND user_id = ?")
                    .bind(room_id.to_string()).bind(user_id)
                    .execute(&mut *tx).await?;
            }
        }
    }
    tx.commit().await?;
    Ok(())
}

// 加载房间的封禁列表
pub async fn load_bans_for_room(pool: &SqlitePool, room_id: Uuid) -> Result<HashSet<String>, AppError> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT user_id FROM room_bans WHERE room_id = ?").bind(room_id.to_string()).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(user_id,)| user_id).collect())
}

// 加载房间的管理员列表
pub async fn load_admins_for_room(pool: &SqlitePool, room_id: Uuid) -> Result<HashSet<String>, AppError> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT user_id FROM room_admins WHERE room_id = ?").bind(room_id.to_string()).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(user_id,)| user_id).collect())
}

// 获取所有房间的基础信息
pub async fn get_all_room_info(pool: &SqlitePool) -> Result<Vec<RoomInfo>, AppError> {
    sqlx::query_as("SELECT id, created_at FROM rooms").fetch_all(pool).await.map_err(Into::into)
}

// 为数据同步获取数据
pub async fn get_data_for_sync(pool: &SqlitePool, room_id: Uuid, details: RoomDetailsResponse) -> Result<DataSyncPayload, AppError> {
    let room_id_str = room_id.to_string();
    let chat_history = sqlx::query_as("SELECT user_id, content, created_at FROM chat_history WHERE room_id = ?").bind(&room_id_str).fetch_all(pool).await?;
    let session_history = sqlx::query_as("SELECT user_id, join_time, leave_time, duration_seconds FROM room_sessions WHERE room_id = ? AND leave_time IS NOT NULL").bind(&room_id_str).fetch_all(pool).await?;

    Ok(DataSyncPayload {
        room_id: details.room_id,
        admin_user_ids: details.admin_user_ids,
        start_time: details.start_time,
        stats: details.stats,
        chat_history,
        session_history,
    })
}

// ====================================================================================
// src/routes.rs - HTTP路由处理
// ====================================================================================
use crate::{
    config::Config,
    db,
    error::AppError,
    handler,
    models::{
        ControlMessage, CreateRoomRequest, CreateRoomResponse, DataSyncPayload, ResetAdminsRequest,
        RoomDetailsResponse, StatsQuery,
    },
    state::AppState,
};
use axum::{
    extract::{ws::WebSocket, Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use headers::{HeaderMap, HeaderValue};
use serde::Deserialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
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
    ws.on_upgrade(move |socket| handler::handle_socket(socket, state, room_id, query.user_id))
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
        .bind(room_id.to_string())
        .bind(&room_name)
        .bind(Utc::now().timestamp())
        .execute(&mut *tx)
        .await?;

    for admin_id in &admin_ids {
        sqlx::query("INSERT INTO room_admins (room_id, user_id) VALUES (?, ?)")
            .bind(room_id.to_string())
            .bind(admin_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    handler::start_room_handler(room_id, state.clone()).await;

    let websocket_url = format!("/ws/rooms/{}", room_id);
    Ok(Json(CreateRoomResponse {
        room_id,
        websocket_url,
    }))
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
        if room_state
            .stats_tx
            .send(StatsQuery { response_tx: tx })
            .await
            .is_ok()
        {
            if let Ok(details) = rx.await {
                details_list.push(details);
            }
        }
    }

    Ok(Json(details_list))
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
        sqlx::query("DELETE FROM room_admins WHERE room_id = ?")
            .bind(room_id.to_string())
            .execute(&mut *tx)
            .await?;
        for admin_id in &payload.admin_user_ids {
            sqlx::query("INSERT INTO room_admins (room_id, user_id) VALUES (?, ?)")
                .bind(room_id.to_string())
                .bind(admin_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;

        // 发送控制消息通知房间处理器更新内存状态
        room.control_tx
            .send(ControlMessage::ResetAdmins(payload.admin_user_ids))
            .await
            .map_err(|_| AppError::InternalServerError)?;
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
            .bind(room_id.to_string())
            .bind(&user_id)
            .execute(&state.db_pool)
            .await?;

        // 发送控制消息
        room.control_tx
            .send(ControlMessage::UnbanUser(user_id))
            .await
            .map_err(|_| AppError::InternalServerError)?;
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
        // 触发数据同步
        if let Some(callback_url) = &state.config.data_callback_url {
            let (tx, rx) = tokio::sync::oneshot::channel();
            if room
                .stats_tx
                .send(StatsQuery { response_tx: tx })
                .await
                .is_ok()
            {
                if let Ok(details) = rx.await {
                    let sync_data =
                        db::get_data_for_sync(&state.db_pool, room_id, details).await?;
                    tokio::spawn(send_data_sync(callback_url.clone(), sync_data));
                }
            }
        }
        drop(room);
    } else {
        return Err(AppError::NotFound(format!("Room {} not found", room_id)));
    }

    Ok(StatusCode::NO_CONTENT)
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

// ====================================================================================
// src/handler.rs - 核心业务逻辑处理器
// ====================================================================================
use crate::{
    db,
    models::{
        ControlMessage, DbWriteCommand, InternalMessage, RoomDetailsResponse, RoomState, RoomStats,
        StatsQuery, WsMessage,
    },
    state::AppState,
};
use axum::extract::ws::{Message as AxumWsMessage, WebSocket};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use uuid::Uuid;

struct ConnectionInfo {
    sender: mpsc::Sender<WsMessage>,
    join_time: Instant,
    is_admin: bool,
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
pub async fn handle_socket(
    mut socket: WebSocket,
    state: Arc<AppState>,
    room_id: Uuid,
    user_id: String,
) {
    let _conn_guard = ConnectionGuard::new(state.total_connections.clone());
    let conn_id = Uuid::new_v4();

    let room_tx = {
        let rooms = state.rooms.lock().await;
        if let Some(room) = rooms.get(&room_id) {
            Some(room.normal_prio_tx.clone())
        } else {
            let _ = socket
                .send(AxumWsMessage::Text(
                    serde_json::to_string(&WsMessage::Error {
                        message: "房间已关闭".to_string(),
                    })
                    .unwrap(),
                ))
                .await;
            return;
        }
    };
    let Some(room_tx) = room_tx else { return; };

    let (tx, mut rx) = mpsc::channel(10);

    if room_tx
        .send(InternalMessage {
            conn_id,
            user_id: user_id.clone(),
            room_id,
            content: WsMessage::UserJoined {
                user_id: user_id.clone(),
            },
            sender: Some(tx.clone()),
        })
        .await
        .is_err()
    {
        tracing::warn!("Failed to register new user, room handler might be down.");
        return;
    }

    tokio::spawn(async move {
        while let Some(msg_to_send) = rx.recv().await {
            let payload = serde_json::to_string(&msg_to_send).unwrap();
            if socket.send(AxumWsMessage::Text(payload)).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = socket.recv().await {
        if let AxumWsMessage::Text(text) = msg {
            if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                if room_tx
                    .send(InternalMessage {
                        conn_id,
                        user_id: user_id.clone(),
                        room_id,
                        content: ws_msg,
                        sender: None,
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        } else if let AxumWsMessage::Close(_) = msg {
            break;
        }
    }

    let _ = room_tx
        .send(InternalMessage {
            conn_id,
            user_id: user_id.clone(),
            room_id,
            content: WsMessage::UserLeft { user_id },
            sender: None,
        })
        .await;
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
    let mut admin_users =
        db::load_admins_for_room(&state.db_pool, room_id)
            .await
            .unwrap_or_default();
    let mut banned_users =
        db::load_bans_for_room(&state.db_pool, room_id)
            .await
            .unwrap_or_default();
    let mut stats = RoomStats::default();
    let start_time = Utc::now().timestamp();

    loop {
        tokio::select! {
            Some(msg) = high_prio_rx.recv() => {
                handle_message(msg, &mut connections, &mut user_id_to_conn_id, &mut muted_users, &mut banned_users, &admin_users, &state, &mut stats).await;
            },
            Some(msg) = normal_prio_rx.recv() => {
                handle_message(msg, &mut connections, &mut user_id_to_conn_id, &mut muted_users, &mut banned_users, &admin_users, &state, &mut stats).await;
            },
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
                let response = RoomDetailsResponse {
                    room_id,
                    admin_user_ids: admin_users.clone(),
                    start_time,
                    stats: stats.clone(),
                };
                let _ = query.response_tx.send(response);
            },
            else => {
                tracing::info!("Room {} handler shutting down.", room_id);
                break;
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
        rooms
            .get(&msg.room_id)
            .map(|r| r.db_writer_tx.clone())
    };
    let Some(db_writer_tx) = db_writer_tx else { return; };

    match msg.content {
        WsMessage::UserJoined { .. } => {
            let sender = msg.sender.expect("UserJoined message must have a sender");

            if banned_users.contains(&msg.user_id) {
                let _ = sender
                    .send(WsMessage::Error {
                        message: "你已被踢出该房间，无法再次加入".to_string(),
                    })
                    .await;
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
            let welcome_msg = WsMessage::WelcomeInfo {
                user_id: msg.user_id.clone(),
                is_muted,
            };
            if sender.send(welcome_msg).await.is_err() {
                return;
            }

            let conn_info = ConnectionInfo {
                sender,
                join_time: Instant::now(),
                is_admin,
            };
            connections.insert(msg.conn_id, conn_info);
            user_id_to_conn_id.insert(msg.user_id.clone(), msg.conn_id);

            stats.current_users += 1;
            stats.total_joins += 1;
            if stats.current_users > stats.peak_users {
                stats.peak_users = stats.current_users;
            }

            let _ = db_writer_tx
                .send(DbWriteCommand::UserJoined {
                    user_id: msg.user_id.clone(),
                    room_id: msg.room_id,
                })
                .await;
            broadcast(
                connections,
                WsMessage::UserJoined {
                    user_id: msg.user_id,
                },
                Some(msg.conn_id),
            )
            .await;
        }
        WsMessage::UserLeft { .. } => {
            if let Some(conn_info) = connections.remove(&msg.conn_id) {
                user_id_to_conn_id.remove(&msg.user_id);
                stats.current_users = stats.current_users.saturating_sub(1);
                let _ = db_writer_tx
                    .send(DbWriteCommand::UserLeft {
                        user_id: msg.user_id.clone(),
                        room_id: msg.room_id,
                        join_time: conn_info.join_time,
                    })
                    .await;
                broadcast(
                    connections,
                    WsMessage::UserLeft {
                        user_id: msg.user_id,
                    },
                    None,
                )
                .await;
            }
        }
        WsMessage::SendMessage { content } => {
            let conn_info = if let Some(info) = connections.get(&msg.conn_id) {
                info
            } else {
                return;
            };
            if muted_users.contains(&msg.user_id) {
                let _ = conn_info.sender.send(WsMessage::YouAreMuted).await;
                return;
            }
            let _ = db_writer_tx
                .send(DbWriteCommand::ChatMessage {
                    user_id: msg.user_id.clone(),
                    room_id: msg.room_id,
                    content: content.clone(),
                })
                .await;
            broadcast(
                connections,
                WsMessage::Message {
                    from: msg.user_id,
                    content,
                    is_admin: conn_info.is_admin,
                },
                None,
            )
            .await;
        }
        WsMessage::KickUser { user_id } => {
            let conn_info = if let Some(info) = connections.get(&msg.conn_id) {
                info
            } else {
                return;
            };
            if !conn_info.is_admin {
                return;
            }

            banned_users.insert(user_id.clone());
            let _ = db_writer_tx
                .send(DbWriteCommand::BanUser {
                    user_id: user_id.clone(),
                    room_id: msg.room_id,
                })
                .await;

            if let Some(target_conn_id) = user_id_to_conn_id.remove(&user_id) {
                if let Some(target_conn) = connections.remove(&target_conn_id) {
                    stats.current_users = stats.current_users.saturating_sub(1);
                    let _ = target_conn.sender.send(WsMessage::YouAreKicked).await;
                }
            }
            broadcast(
                connections,
                WsMessage::System {
                    message: format!("用户 {} 已被踢出房间", user_id),
                },
                None,
            )
            .await;
        }
        WsMessage::MuteUser { user_id } => {
            let conn_info = if let Some(info) = connections.get(&msg.conn_id) {
                info
            } else {
                return;
            };
            if !conn_info.is_admin {
                return;
            }
            muted_users.insert(user_id.clone());
            broadcast(connections, WsMessage::UserMuted { user_id }, None).await;
        }
        _ => {}
    }
}

// 广播消息给房间内的所有用户
async fn broadcast(
    connections: &HashMap<Uuid, ConnectionInfo>,
    msg: WsMessage,
    exclude_conn_id: Option<Uuid>,
) {
    for (conn_id, conn_info) in connections.iter() {
        if Some(*conn_id) == exclude_conn_id {
            continue;
        }
        if conn_info.sender.send(msg.clone()).await.is_err() {}
    }
}
