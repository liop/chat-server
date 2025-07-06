
// ====================================================================================
// src/main.rs - 应用入口
// ====================================================================================
use axum::{
    routing::{delete, get, put},
    Router,
};
use sqlx::sqlite::SqlitePoolOptions;
use std::net::SocketAddr;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod db;
mod error;
mod handler;
mod models;
mod routes;
mod state;
mod sync;

use config::Config;
use state::AppState;
use sync::SyncService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志记录
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "chat_server=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 加载配置
    let config = Config::from_env().expect("Failed to load config");
    let db_config = config.clone();
    let app_state_config = config.clone();

    // 连接数据库并运行迁移
    tracing::info!("正在连接数据库: {}", db_config.database_url);
    let pool = SqlitePoolOptions::new()
        .max_connections(10) // 增加连接池大小以支持更多并发API请求
        .connect(&db_config.database_url)
        .await
        .expect("Failed to connect to database");
    
    tracing::info!("数据库连接成功，正在运行迁移...");
    db::migrate(&pool).await.expect("Failed to run migrations");
    tracing::info!("数据库迁移完成");

    // 创建共享的应用状态
    let app_state = Arc::new(AppState {
        db_pool: pool.clone(),
        rooms: Mutex::new(std::collections::HashMap::new()),
        total_connections: Arc::new(AtomicU32::new(0)),
        config: app_state_config,
    });

    // 启动同步服务
    let sync_service = SyncService::new(app_state.clone(), config.clone());
    sync_service.start_periodic_sync().await;

    // 定义CORS策略
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 创建Axum路由
    let app = Router::new()
        .route("/management/health", get(routes::health_check))
        .route("/management/rooms", get(routes::list_rooms).post(routes::create_room))
        .route(
            "/management/rooms/:room_id",
            delete(routes::close_room),
        )
        .route(
            "/management/rooms/:room_id/admins",
            put(routes::reset_admins),
        )
        .route(
            "/management/rooms/:room_id/bans/:user_id",
            delete(routes::unban_user),
        )
        .route("/ws/rooms/:room_id", get(routes::ws_handler))
        .route("/management/sync", get(routes::get_sync_data).post(routes::trigger_sync))
        .with_state(app_state)
        .layer(cors);

    // 启动服务器
    let addr: SocketAddr = config
        .bind_address
        .parse()
        .expect("Invalid bind address");
    tracing::debug!("服务器正在监听于 {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();

    Ok(())
}