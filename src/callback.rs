// ====================================================================================
// src/callback.rs - 回调服务
// ====================================================================================
use crate::{
    config::Config,
    db,
    models::{CallbackEvent, RoomBasicInfo, RoomStats},
};
use reqwest::Client;
use tokio::time::{sleep, Duration};
use tracing;
use uuid::Uuid;

pub struct CallbackService {
    config: Config,
    client: Client,
}

impl CallbackService {
    pub fn new(config: Config) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.callback_timeout_seconds))
            .build()
            .unwrap_or_default();
        
        Self { config, client }
    }

    // 发送房间事件回调
    pub async fn send_room_event(&self, event: CallbackEvent) {
        if let Some(url) = &self.config.room_event_callback_url {
            self.send_callback_with_retry(url, &event).await;
        }
    }

    // 发送聊天记录批次回调
    pub async fn send_chat_history_batch(&self, room_id: Uuid, messages: Vec<db::ChatHistoryEntry>, batch_id: String, is_last_batch: bool) {
        if let Some(url) = &self.config.chat_history_callback_url {
            let event = CallbackEvent::ChatHistoryBatch {
                room_id,
                messages,
                batch_id,
                is_last_batch,
                timestamp: chrono::Utc::now().timestamp(),
            };
            self.send_callback_with_retry(url, &event).await;
        }
    }

    // 发送会话历史批次回调
    pub async fn send_session_history_batch(&self, room_id: Uuid, sessions: Vec<db::SessionHistoryEntry>, batch_id: String, is_last_batch: bool) {
        if let Some(url) = &self.config.session_history_callback_url {
            let event = CallbackEvent::SessionHistoryBatch {
                room_id,
                sessions,
                batch_id,
                is_last_batch,
                timestamp: chrono::Utc::now().timestamp(),
            };
            self.send_callback_with_retry(url, &event).await;
        }
    }

    // 发送定时同步回调
    pub async fn send_periodic_sync(&self, room_id: Uuid, room_info: RoomBasicInfo) {
        if let Some(url) = &self.config.periodic_sync_callback_url {
            let event = CallbackEvent::PeriodicSync {
                room_id,
                room_info,
                last_sync_time: chrono::Utc::now().timestamp(),
            };
            self.send_callback_with_retry(url, &event).await;
        }
    }

    // 发送房间创建事件
    pub async fn send_room_created(&self, room_id: Uuid, room_name: String, admin_ids: Vec<String>) {
        let event = CallbackEvent::RoomCreated {
            room_id,
            room_name,
            admin_ids,
            timestamp: chrono::Utc::now().timestamp(),
        };
        self.send_room_event(event).await;
    }

    // 发送房间关闭事件
    pub async fn send_room_closed(&self, room_id: Uuid, final_stats: RoomStats) {
        let event = CallbackEvent::RoomClosed {
            room_id,
            final_stats,
            timestamp: chrono::Utc::now().timestamp(),
        };
        self.send_room_event(event).await;
    }

     
    // 带重试机制的回调发送
    async fn send_callback_with_retry(&self, url: &str, event: &CallbackEvent) {
        let mut retries = 0;
        let max_retries = self.config.callback_max_retries;
        let retry_delay = Duration::from_secs(self.config.callback_retry_delay_seconds);

        while retries <= max_retries {
            match self.client.post(url).json(event).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        tracing::info!("成功发送回调到 {}: {:?}", url, event);
                        return;
                    } else {
                        tracing::warn!("回调请求失败，状态码: {}，URL: {}", response.status(), url);
                    }
                }
                Err(e) => {
                    tracing::error!("发送回调失败到 {}: {} (重试 {}/{})", url, e, retries, max_retries);
                }
            }

            retries += 1;
            if retries <= max_retries {
                sleep(retry_delay).await;
            }
        }

        tracing::error!("回调发送最终失败，URL: {}，事件: {:?}", url, event);
    }     
} 