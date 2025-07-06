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