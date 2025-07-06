
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
    pub sync_interval_seconds: u64,
    
    // 拆分后的回调URL配置
    pub room_event_callback_url: Option<String>,
    pub chat_history_callback_url: Option<String>,
    pub session_history_callback_url: Option<String>,
    pub periodic_sync_callback_url: Option<String>,
    
    // 回调配置
    pub chat_history_batch_size: u32,
    pub session_history_batch_size: u32,
    pub chat_history_batch_interval_seconds: u64,
    pub session_history_batch_interval_seconds: u64,
    
    // 回调重试配置
    pub callback_max_retries: u32,
    pub callback_retry_delay_seconds: u64,
    pub callback_timeout_seconds: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, dotenvy::Error> {
        // 尝试加载 .env 文件，如果失败则忽略（可能文件不存在）
        if let Err(e) = dotenvy::dotenv() {
            eprintln!("Warning: Failed to load .env file: {}", e);
        }
        
        // 数据库URL现在是可选的，默认使用内存数据库
        
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite::memory:".to_string()),
            bind_address: std::env::var("BIND_ADDRESS")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            admin_api_key: std::env::var("ADMIN_API_KEY")
                .expect("ADMIN_API_KEY must be set"),
            data_callback_url: std::env::var("DATA_CALLBACK_URL").ok(),
            max_connections: std::env::var("MAX_CONNECTIONS")
                .unwrap_or_else(|_| "100000".to_string())
                .parse()
                .expect("MAX_CONNECTIONS must be a valid number"),
            sync_interval_seconds: std::env::var("SYNC_INTERVAL_SECONDS")
                .unwrap_or_else(|_| "300".to_string()) // 默认5分钟
                .parse()
                .expect("SYNC_INTERVAL_SECONDS must be a valid number"),
            
            // 拆分后的回调URL配置
            room_event_callback_url: std::env::var("ROOM_EVENT_CALLBACK_URL").ok(),
            chat_history_callback_url: std::env::var("CHAT_HISTORY_CALLBACK_URL").ok(),
            session_history_callback_url: std::env::var("SESSION_HISTORY_CALLBACK_URL").ok(),
            periodic_sync_callback_url: std::env::var("PERIODIC_SYNC_CALLBACK_URL").ok(),
            
            // 回调配置
            chat_history_batch_size: std::env::var("CHAT_HISTORY_BATCH_SIZE")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .expect("CHAT_HISTORY_BATCH_SIZE must be a valid number"),
            session_history_batch_size: std::env::var("SESSION_HISTORY_BATCH_SIZE")
                .unwrap_or_else(|_| "500".to_string())
                .parse()
                .expect("SESSION_HISTORY_BATCH_SIZE must be a valid number"),
            chat_history_batch_interval_seconds: std::env::var("CHAT_HISTORY_BATCH_INTERVAL_SECONDS")
                .unwrap_or_else(|_| "300".to_string()) // 默认5分钟
                .parse()
                .expect("CHAT_HISTORY_BATCH_INTERVAL_SECONDS must be a valid number"),
            session_history_batch_interval_seconds: std::env::var("SESSION_HISTORY_BATCH_INTERVAL_SECONDS")
                .unwrap_or_else(|_| "600".to_string()) // 默认10分钟
                .parse()
                .expect("SESSION_HISTORY_BATCH_INTERVAL_SECONDS must be a valid number"),
            
            // 回调重试配置
            callback_max_retries: std::env::var("CALLBACK_MAX_RETRIES")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .expect("CALLBACK_MAX_RETRIES must be a valid number"),
            callback_retry_delay_seconds: std::env::var("CALLBACK_RETRY_DELAY_SECONDS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .expect("CALLBACK_RETRY_DELAY_SECONDS must be a valid number"),
            callback_timeout_seconds: std::env::var("CALLBACK_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .expect("CALLBACK_TIMEOUT_SECONDS must be a valid number"),
        })
    }
}

