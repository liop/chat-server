
// ====================================================================================
// src/models.rs - 数据模型定义
// ====================================================================================
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tokio::sync::mpsc;
use uuid::Uuid;
use crate::db;

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
