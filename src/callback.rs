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

    // 发送用户加入事件
    pub async fn send_user_joined(&self, room_id: Uuid, user_id: String) {
        let event = CallbackEvent::UserJoined {
            room_id,
            user_id,
            timestamp: chrono::Utc::now().timestamp(),
        };
        self.send_room_event(event).await;
    }

    // 发送用户离开事件
    pub async fn send_user_left(&self, room_id: Uuid, user_id: String, duration: i64) {
        let event = CallbackEvent::UserLeft {
            room_id,
            user_id,
            duration,
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

    // 批量发送聊天记录
    pub async fn send_chat_history_in_batches(&self, pool: &sqlx::SqlitePool, room_id: Uuid) {
        let batch_size = self.config.chat_history_batch_size;
        let mut page = 1;
        let mut total_sent = 0;

        loop {
            let query = crate::models::PaginationQuery {
                page: Some(page),
                limit: Some(batch_size),
                from: None,
                to: None,
            };

            match db::get_chat_history_page(pool, room_id, &query).await {
                Ok(chat_page) => {
                    let is_last_batch = !chat_page.pagination.has_next;
                    let batch_id = format!("chat_{}_{}", room_id, page);
                    let records_len = chat_page.records.len();
                    
                    self.send_chat_history_batch(
                        room_id,
                        chat_page.records,
                        batch_id,
                        is_last_batch,
                    ).await;

                    total_sent += records_len;
                    
                    if is_last_batch {
                        tracing::info!("完成聊天记录批量发送，房间: {}，总记录数: {}", room_id, total_sent);
                        break;
                    }
                    
                    page += 1;
                }
                Err(e) => {
                    tracing::error!("获取聊天记录分页失败: {}", e);
                    break;
                }
            }
        }
    }

    // 批量发送会话历史
    pub async fn send_session_history_in_batches(&self, pool: &sqlx::SqlitePool, room_id: Uuid) {
        let batch_size = self.config.session_history_batch_size;
        let mut page = 1;
        let mut total_sent = 0;

        loop {
            let query = crate::models::PaginationQuery {
                page: Some(page),
                limit: Some(batch_size),
                from: None,
                to: None,
            };

            match db::get_session_history_page(pool, room_id, &query).await {
                Ok(session_page) => {
                    let is_last_batch = !session_page.pagination.has_next;
                    let batch_id = format!("session_{}_{}", room_id, page);
                    let records_len = session_page.records.len();
                    
                    self.send_session_history_batch(
                        room_id,
                        session_page.records,
                        batch_id,
                        is_last_batch,
                    ).await;

                    total_sent += records_len;
                    
                    if is_last_batch {
                        tracing::info!("完成会话历史批量发送，房间: {}，总记录数: {}", room_id, total_sent);
                        break;
                    }
                    
                    page += 1;
                }
                Err(e) => {
                    tracing::error!("获取会话历史分页失败: {}", e);
                    break;
                }
            }
        }
    }
} 