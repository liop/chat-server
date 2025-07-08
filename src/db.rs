
// ====================================================================================
// src/db.rs - 数据库交互
// ====================================================================================
use crate::error::AppError;
use crate::models::{DataSyncPayload, DbWriteCommand, RoomDetailsResponse, RoomBasicInfo};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(FromRow, Serialize, Debug)]
pub struct ChatHistoryEntry {
    pub user_id: String,
    pub nickname: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(FromRow, Serialize, Debug)]
pub struct SessionHistoryEntry {
    pub user_id: String,
    pub nickname: String,
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
        CREATE TABLE IF NOT EXISTS chat_history (id INTEGER PRIMARY KEY AUTOINCREMENT, room_id TEXT NOT NULL, user_id TEXT NOT NULL, nickname TEXT NOT NULL, content TEXT NOT NULL, created_at INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS room_sessions (id INTEGER PRIMARY KEY AUTOINCREMENT, room_id TEXT NOT NULL, user_id TEXT NOT NULL, nickname TEXT NOT NULL, join_time INTEGER NOT NULL, leave_time INTEGER, duration_seconds INTEGER);
        CREATE TABLE IF NOT EXISTS room_bans (room_id TEXT NOT NULL, user_id TEXT NOT NULL, PRIMARY KEY (room_id, user_id));
        
        -- 添加索引以提高查询性能
        CREATE INDEX IF NOT EXISTS idx_chat_history_room_id ON chat_history(room_id);
        CREATE INDEX IF NOT EXISTS idx_chat_history_created_at ON chat_history(created_at);
        CREATE INDEX IF NOT EXISTS idx_session_history_room_id ON room_sessions(room_id);
        CREATE INDEX IF NOT EXISTS idx_session_history_join_time ON room_sessions(join_time);
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
            DbWriteCommand::UserJoined { user_id, nickname, room_id } => {
                sqlx::query("INSERT INTO room_sessions (room_id, user_id, nickname, join_time) VALUES (?, ?, ?, ?)")
                    .bind(room_id.to_string()).bind(user_id).bind(nickname).bind(chrono::Utc::now().timestamp())
                    .execute(&mut *tx).await?;
            }
            DbWriteCommand::UserLeft { user_id, nickname: _, room_id, join_time } => {
                let duration = join_time.elapsed().as_secs() as i64;
                sqlx::query("UPDATE room_sessions SET leave_time = ?, duration_seconds = ? WHERE user_id = ? AND room_id = ? AND leave_time IS NULL")
                    .bind(chrono::Utc::now().timestamp()).bind(duration).bind(user_id).bind(room_id.to_string())
                    .execute(&mut *tx).await?;
            }
            DbWriteCommand::ChatMessage { user_id, nickname, room_id, content } => {
                sqlx::query("INSERT INTO chat_history (room_id, user_id, nickname, content, created_at) VALUES (?, ?, ?, ?, ?)")
                    .bind(room_id.to_string()).bind(user_id).bind(nickname).bind(content).bind(chrono::Utc::now().timestamp())
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
    // 确保表存在（针对内存数据库）
    migrate(pool).await?;
    
    let rows: Vec<(String,)> = sqlx::query_as("SELECT user_id FROM room_bans WHERE room_id = ?").bind(room_id.to_string()).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(user_id,)| user_id).collect())
}

// 加载房间的管理员列表
pub async fn load_admins_for_room(pool: &SqlitePool, room_id: Uuid) -> Result<HashSet<String>, AppError> {
    // 确保表存在（针对内存数据库）
    migrate(pool).await?;
    
    let rows: Vec<(String,)> = sqlx::query_as("SELECT user_id FROM room_admins WHERE room_id = ?").bind(room_id.to_string()).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(user_id,)| user_id).collect())
}

// 获取房间基础信息
pub async fn get_room_basic_info(pool: &SqlitePool, room_id: Uuid) -> Result<Option<RoomBasicInfo>, AppError> {
    let room_id_str = room_id.to_string();
    
    // 获取房间基本信息
    let room_row = sqlx::query_as::<_, (String, i64)>("SELECT name, created_at FROM rooms WHERE id = ?")
        .bind(&room_id_str)
        .fetch_optional(pool)
        .await?;
    
    if let Some((room_name, created_at)) = room_row {
        // 获取管理员列表
        let admin_rows = sqlx::query_as::<_, (String,)>("SELECT user_id FROM room_admins WHERE room_id = ?")
            .bind(&room_id_str)
            .fetch_all(pool)
            .await?;
        let admin_user_ids: HashSet<String> = admin_rows.into_iter().map(|(user_id,)| user_id).collect();
        
        // 获取最后活动时间（最新聊天消息时间）
        let last_activity = sqlx::query_as::<_, (i64,)>("SELECT MAX(created_at) FROM chat_history WHERE room_id = ?")
            .bind(&room_id_str)
            .fetch_optional(pool)
            .await?
            .map(|(timestamp,)| timestamp)
            .unwrap_or(created_at);
        
        Ok(Some(RoomBasicInfo {
            room_id,
            room_name,
            admin_user_ids,
            current_connections: 0, // 这个需要从内存状态获取
            created_at,
            last_activity,
        }))
    } else {
        Ok(None)
    }
}
 
// 为数据同步获取数据（保持向后兼容）
pub async fn get_data_for_sync(pool: &SqlitePool, room_id: Uuid, details: RoomDetailsResponse) -> Result<DataSyncPayload, AppError> {
    let room_id_str = room_id.to_string();
    let chat_history = sqlx::query_as("SELECT user_id, nickname, content, created_at FROM chat_history WHERE room_id = ?").bind(&room_id_str).fetch_all(pool).await?;
    let session_history = sqlx::query_as("SELECT user_id, nickname, join_time, leave_time, duration_seconds FROM room_sessions WHERE room_id = ? AND leave_time IS NOT NULL").bind(&room_id_str).fetch_all(pool).await?;

    Ok(DataSyncPayload {
        room_id: details.room_id,
        admin_user_ids: details.admin_user_ids,
        start_time: details.start_time,
        stats: details.stats,
        chat_history,
        session_history,
    })
}
