/*
 * ====================================================================================
 * README.md - 项目说明
 * ====================================================================================
 *
 * 这是一个使用Rust编写的高性能、按需启动的匿名聊天室后端服务。
 *
 * ## 功能特性
 * - 基于Axum和Tokio，纯异步实现，支持大规模并发连接。
 * - 使用SQLite作为嵌入式数据库（推荐内存模式），无需外部依赖，易于部署。
 * - 提供完整的HTTP管理API，用于被外部服务控制（创建/查询/重置管理员/解禁/关闭房间）。
 * - 支持多房间、多管理员、踢人、禁言等功能。
 * - 支持通过环境变量设置最大连接数，当达到上限时拒绝新连接，实现负载保护。
 * - 权限校验（管理员、封禁）在内存中完成，移除了连接时的数据库查询，以应对海量用户瞬时涌入。
 * - 踢出用户后，该用户将被永久禁止加入该房间，但可通过API解禁。
 * - 内置管理员消息高优先级处理机制。
 * - 采用异步批处理优化数据库写入，应对高频用户进出。
 * - 支持在房间关闭后，将包含统计信息的完整数据同步到外部回调URL。
 *
 * ## 如何运行
 *
 * 1. **安装 Rust**: 如果你还没有安装，请访问 https://www.rust-lang.org/
 *
 * 2. **创建项目**:
 * ```bash
 * cargo new chat_server
 * cd chat_server
 * ```
 *
 * 3. **配置 `Cargo.toml`**:
 * 将下面的 `Cargo.toml` 内容复制到你的 `Cargo.toml` 文件中。
 *
 * 4. **复制代码**:
 * 将下面的所有 `src/*.rs` 文件内容复制到你的 `src` 目录下对应的文件中。
 *
 * 5. **设置环境变量**:
 * 在运行前，需要设置以下环境变量：
 * ```bash
 * # 服务监听地址
 * export BIND_ADDRESS="0.0.0.0:3000"
 * # 推荐使用内存数据库以获得最佳性能和自动清理
 * export DATABASE_URL="sqlite::memory:"
 * # 或者使用文件数据库
 * # export DATABASE_URL="sqlite:chat_server.db"
 * # 管理API的安全密钥
 * export ADMIN_API_KEY="your-super-secret-key"
 * # 设置服务器最大连接数上限
 * export MAX_CONNECTIONS=100000
 * # (可选) 房间关闭后数据同步的回调URL
 * export DATA_CALLBACK_URL="http://your-java-service/api/chat_data_sync"
 * ```
 *
 * 6. **编译并运行**:
 * ```bash
 * cargo run --release
 * ```
 *
 * ## API 使用
 *
 * - **健康检查**: `GET http://localhost:3000/management/health`
 * - **创建房间**: `POST http://localhost:3000/management/rooms`
 * - **查询所有房间**: `GET http://localhost:3000/management/rooms`
 * - **重置管理员**: `PUT http://localhost:3000/management/rooms/{room_id}/admins`
 * - **解除用户封禁**: `DELETE http://localhost:3000/management/rooms/{room_id}/bans/{user_id}`
 * - **关闭房间**: `DELETE http://localhost:3000/management/rooms/{room_id}`
 * - **加入房间 (WebSocket)**: `ws://localhost:3000/ws/rooms/{room_id}?user_id=user123`
 *
 * (所有管理API都需要在Header中提供 `X-Api-Key: your-super-secret-key`)
 *
 */

/*
 * ====================================================================================
 * Cargo.toml - 项目依赖配置
 * ====================================================================================
 */
/*
[package]
name = "chat_server"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = { version = "0.7", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.23"
futures-util = { version = "0.3", default-features = false, features = ["sink", "std"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "uuid", "time"] }
uuid = { version = "1.8", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "1.0"
dotenvy = "0.15"
reqwest = { version = "0.12", features = ["json"] }
http = "1.1"
headers = "0.4"
tower-http = { version = "0.5.0", features = ["cors"] }
*/





