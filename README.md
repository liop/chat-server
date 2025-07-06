# Rust 聊天服务器

一个基于 Rust 和 Axum 的高性能聊天服务器，支持 WebSocket 实时通信。

## 特性

- 🚀 高性能异步处理
- 💬 实时 WebSocket 聊天
- 👥 房间管理
- 🔐 管理员权限控制
- 🚫 用户封禁功能
- 📊 房间统计
- 💾 数据持久化（可选）
- 🔄 智能数据同步策略

## 快速开始

### 环境变量配置

创建 `.env` 文件（可选）：

```bash
# 数据库配置（可选，默认使用内存数据库）
DATABASE_URL=sqlite::memory:

# 服务器配置
BIND_ADDRESS=0.0.0.0:3000
ADMIN_API_KEY=your_secret_api_key

# 可选配置
MAX_CONNECTIONS=100000
DATA_CALLBACK_URL=http://example.com/callback

# 数据同步配置
SYNC_INTERVAL_SECONDS=300  # 定时同步间隔（秒），默认5分钟
```

### 运行服务器

```bash
# 编译并运行
cargo run

# 或者先编译
cargo build --release
./target/release/rust-demo
```

## API 接口

### 创建房间

```bash
curl -X POST http://localhost:3000/management/rooms \
  -H "Content-Type: application/json" \
  -H "X-Api-Key: your_secret_api_key" \
  -d '{
    "room_name": "测试房间",
    "admin_user_ids": ["admin1", "admin2"]
  }'
```

**注意：** 创建房间时会自动触发一次数据同步。

### 查询所有房间

```bash
curl -X GET http://localhost:3000/management/rooms \
  -H "X-Api-Key: your_secret_api_key"
```

### WebSocket 连接

```
ws://localhost:3000/ws/rooms/{room_id}?user_id={user_id}
```

### 数据同步接口

#### 获取所有房间的同步数据

```bash
curl -X GET http://localhost:3000/management/sync \
  -H "X-Api-Key: your_secret_api_key"
```

#### 手动触发同步

```bash
curl -X POST http://localhost:3000/management/sync \
  -H "X-Api-Key: your_secret_api_key"
```

## 数据同步策略

系统支持三种数据同步方式：

### 1. 创建房间时同步
每次创建新房间时，系统会自动触发一次数据同步，将房间信息发送到配置的外部系统。

### 2. 定时同步
系统会根据 `SYNC_INTERVAL_SECONDS` 配置的时间间隔，定期同步所有房间的数据。

### 3. 外部系统主动拉取
外部系统可以通过 `/management/sync` GET 接口主动获取所有房间的同步数据。

## 数据库配置

### 内存数据库（默认）

如果不设置 `DATABASE_URL` 环境变量，系统将自动使用 SQLite 内存数据库：

```bash
# 不需要设置 DATABASE_URL，系统会自动使用内存数据库
ADMIN_API_KEY=your_secret_api_key
SYNC_INTERVAL_SECONDS=300
```

**注意：** 内存数据库在服务器重启后数据会丢失。

### 文件数据库

如果需要数据持久化，可以设置文件数据库：

```bash
DATABASE_URL=sqlite:./chat_server.db
ADMIN_API_KEY=your_secret_api_key
SYNC_INTERVAL_SECONDS=300
```

## 开发

```bash
# 安装依赖
cargo build

# 运行测试
cargo test

# 代码格式化
cargo fmt

# 代码检查
cargo clippy
```

## 项目结构

```
src/
├── main.rs          # 应用入口
├── config.rs        # 配置管理
├── db.rs           # 数据库操作
├── error.rs        # 错误处理
├── handler.rs      # WebSocket 处理器
├── models.rs       # 数据模型
├── routes.rs       # HTTP 路由
├── state.rs        # 应用状态
└── sync.rs         # 数据同步服务
```
