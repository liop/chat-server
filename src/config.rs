
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
        })
    }
}

