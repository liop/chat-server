
// ====================================================================================
// src/models.rs - 数据模型定义
// ====================================================================================
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, HashMap};
use tokio::sync::mpsc;
use uuid::Uuid;
use crate::db;
use serde_json::Value;

// WebSocket消息模型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    SendMessage { content: String },
    KickUser { user_id: String },
    MuteUser { user_id: String },
    Message { from: String, nickname: String, content: String, is_admin: bool },
    UserJoined { user_id: String, nickname: String },
    UserLeft { user_id: String, nickname: String },
    YouAreKicked,
    YouAreMuted,
    UserMuted { user_id: String },
    Error { message: String },
    System { message: String },
    WelcomeInfo { user_id: String, nickname: String, is_muted: bool },
    RoomStats { current_users: u32, peak_users: u32 },
    CustomEvent { event_type: String, payload: Value },
}

// 房间内部状态
#[derive(Debug)]
pub struct RoomState {
    pub high_prio_tx: mpsc::Sender<InternalMessage>,
    pub normal_prio_tx: mpsc::Sender<InternalMessage>,
    pub db_writer_tx: mpsc::Sender<DbWriteCommand>,
    pub control_tx: mpsc::Sender<ControlMessage>,
    pub stats_tx: mpsc::Sender<StatsQuery>,
    pub user_last_message_time: HashMap<String, i64>, // 新增：用户上次发言时间戳
    pub pending_join_notify: bool, // 新增：是否有待推送的用户加入通知
    pub join_notify_timer_active: bool, // 新增：定时器是否激活
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
    pub nickname: String,
    pub room_id: Uuid,
    pub content: WsMessage,
    pub sender: Option<mpsc::Sender<WsMessage>>,
}

// 数据库写入命令
#[derive(Debug)]
pub enum DbWriteCommand {
    UserJoined { user_id: String, nickname: String, room_id: Uuid },
    UserLeft { user_id: String, nickname: String, room_id: Uuid, join_time: std::time::Instant },
    ChatMessage { user_id: String, nickname: String, room_id: Uuid, content: String },
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
    pub room_name: String,
    pub admin_user_ids: HashSet<String>,
    pub start_time: i64,
    #[serde(flatten)]
    pub stats: RoomStats,
}

// 拆分后的数据模型
#[derive(Serialize, Clone, Debug)]
pub struct RoomBasicInfo {
    pub room_id: Uuid,
    pub room_name: String,
    pub admin_user_ids: HashSet<String>,
    pub current_connections: u32,
    pub created_at: i64,
    pub last_activity: i64,
}
 

// 回调事件类型
#[derive(Serialize, Debug)]
#[serde(tag = "event_type")]
pub enum CallbackEvent {
    // 房间事件（实时）
    RoomCreated {
        room_id: Uuid,
        room_name: String,
        admin_ids: Vec<String>,
        timestamp: i64,
    },
    RoomClosed {
        room_id: Uuid,
        final_stats: RoomStats,
        timestamp: i64,
    },
    UserJoined {
        room_id: Uuid,
        user_id: String,
        timestamp: i64,
    },
    UserLeft {
        room_id: Uuid,
        user_id: String,
        duration: i64,
        timestamp: i64,
    },
    
    // 聊天记录（批量）
    ChatHistoryBatch {
        room_id: Uuid,
        messages: Vec<db::ChatHistoryEntry>,
        batch_id: String,
        is_last_batch: bool,
        timestamp: i64,
    },
    
    // 会话历史（批量）
    SessionHistoryBatch {
        room_id: Uuid,
        sessions: Vec<db::SessionHistoryEntry>,
        batch_id: String,
        is_last_batch: bool,
        timestamp: i64,
    },
    
    // 定时同步（完整房间信息）
    PeriodicSync {
        room_id: Uuid,
        room_info: RoomBasicInfo,
        last_sync_time: i64,
    },
}

// 保持向后兼容的旧数据结构
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
