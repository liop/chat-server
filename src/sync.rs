// ====================================================================================
// src/sync.rs - 数据同步服务
// ====================================================================================
use crate::{
    config::Config,
    db,
    models::{DataSyncPayload, StatsQuery},
    state::AppState,
};
use std::sync::Arc;
use tokio::time::{self, Duration};
use tracing;
use uuid::Uuid;

pub struct SyncService {
    state: Arc<AppState>,
    config: Config,
}

impl SyncService {
    pub fn new(state: Arc<AppState>, config: Config) -> Self {
        Self { state, config }
    }

    // 启动定时同步任务
    pub async fn start_periodic_sync(&self) {
        let state = self.state.clone();
        let config = self.config.clone();
        let interval = Duration::from_secs(config.sync_interval_seconds);

        tracing::info!("启动定时同步服务，间隔: {}秒", config.sync_interval_seconds);

        tokio::spawn(async move {
            let mut interval_timer = time::interval(interval);
            loop {
                interval_timer.tick().await;
                if let Err(e) = Self::sync_all_rooms(&state, &config).await {
                    tracing::error!("定时同步失败: {}", e);
                }
            }
        });
    }

    // 同步指定房间
    pub async fn sync_room(room_id: Uuid, state: &Arc<AppState>, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        let rooms = state.rooms.lock().await;
        
        if let Some(room) = rooms.get(&room_id) {
            let (tx, rx) = tokio::sync::oneshot::channel();
            if room.stats_tx.send(StatsQuery { response_tx: tx }).await.is_ok() {
                if let Ok(details) = rx.await {
                    let sync_data = db::get_data_for_sync(&state.db_pool, room_id, details).await?;
                    Self::send_sync_data(&config.data_callback_url, sync_data).await;
                }
            }
        }
        
        Ok(())
    }

    // 同步所有房间
    async fn sync_all_rooms(state: &Arc<AppState>, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        let mut sync_tasks = Vec::new();
        
        // 收集所有房间信息，避免生命周期问题
        let room_info: Vec<(Uuid, tokio::sync::mpsc::Sender<StatsQuery>)> = {
            let rooms = state.rooms.lock().await;
            rooms.iter()
                .map(|(room_id, room)| (*room_id, room.stats_tx.clone()))
                .collect()
        };

        for (room_id, stats_tx) in room_info {
            let state = state.clone();
            let config = config.clone();
            
            let task = tokio::spawn(async move {
                let (tx, rx) = tokio::sync::oneshot::channel();
                if stats_tx.send(StatsQuery { response_tx: tx }).await.is_ok() {
                    if let Ok(details) = rx.await {
                        if let Ok(sync_data) = db::get_data_for_sync(&state.db_pool, room_id, details).await {
                            Self::send_sync_data(&config.data_callback_url, sync_data).await;
                        }
                    }
                }
            });
            
            sync_tasks.push(task);
        }

        // 等待所有同步任务完成
        for task in sync_tasks {
            if let Err(e) = task.await {
                tracing::error!("同步任务失败: {}", e);
            }
        }

        tracing::info!("完成所有房间的定时同步");
        Ok(())
    }

    // 发送同步数据到外部系统
    async fn send_sync_data(callback_url: &Option<String>, payload: DataSyncPayload) {
        if let Some(url) = callback_url {
            let client = reqwest::Client::new();
            match client.post(url).json(&payload).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        tracing::info!("成功发送数据同步到 {}，房间: {}", url, payload.room_id);
                    } else {
                        tracing::warn!("数据同步请求失败，状态码: {}，房间: {}", response.status(), payload.room_id);
                    }
                }
                Err(e) => {
                    tracing::error!("发送数据同步失败到 {}: {}，房间: {}", url, e, payload.room_id);
                }
            }
        }
    }

    // 获取所有房间的同步数据（供外部系统主动拉取）
    pub async fn get_all_sync_data(state: &Arc<AppState>) -> Result<Vec<DataSyncPayload>, Box<dyn std::error::Error>> {
        let mut sync_data_list = Vec::new();
        
        // 收集所有房间信息，避免生命周期问题
        let room_info: Vec<(Uuid, tokio::sync::mpsc::Sender<StatsQuery>)> = {
            let rooms = state.rooms.lock().await;
            rooms.iter()
                .map(|(room_id, room)| (*room_id, room.stats_tx.clone()))
                .collect()
        };

        for (room_id, stats_tx) in room_info {
            let (tx, rx) = tokio::sync::oneshot::channel();
            if stats_tx.send(StatsQuery { response_tx: tx }).await.is_ok() {
                if let Ok(details) = rx.await {
                    let sync_data = db::get_data_for_sync(&state.db_pool, room_id, details).await?;
                    sync_data_list.push(sync_data);
                }
            }
        }

        Ok(sync_data_list)
    }

    // 手动触发同步
    pub async fn trigger_manual_sync(state: &Arc<AppState>, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("手动触发数据同步");
        Self::sync_all_rooms(state, config).await
    }
} 