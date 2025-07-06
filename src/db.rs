
// ====================================================================================
// src/db.rs - 数据库交互
// ====================================================================================
use crate::error::AppError;
use crate::models::{DataSyncPayload, DbWriteCommand, RoomDetailsResponse};
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
                    .bind(room_id.to_string()).bind(user_id).bind(chrono::Utc::now().timestamp())
                    .execute(&mut *tx).await?;
            }
            DbWriteCommand::UserLeft { user_id, room_id, join_time } => {
                let duration = join_time.elapsed().as_secs() as i64;
                sqlx::query("UPDATE room_sessions SET leave_time = ?, duration_seconds = ? WHERE user_id = ? AND room_id = ? AND leave_time IS NULL")
                    .bind(chrono::Utc::now().timestamp()).bind(duration).bind(user_id).bind(room_id.to_string())
                    .execute(&mut *tx).await?;
            }
            DbWriteCommand::ChatMessage { user_id, room_id, content } => {
                sqlx::query("INSERT INTO chat_history (room_id, user_id, content, created_at) VALUES (?, ?, ?, ?)")
                    .bind(room_id.to_string()).bind(user_id).bind(content).bind(chrono::Utc::now().timestamp())
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
