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
- 📤 拆分式回调系统
- ⏱️ 消息频率限制与截流（普通用户发言限流，用户加入通知1秒合并推送）
- 🛡️ 高优先级/低优先级消息分片调度，保证系统响应性
- 🧩 管理员自定义事件（如福袋、红包）高优先级推送

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

# 传统同步配置（保持向后兼容）
DATA_CALLBACK_URL=http://example.com/callback

# 拆分后的回调URL配置
ROOM_EVENT_CALLBACK_URL=http://example.com/api/room-events
CHAT_HISTORY_CALLBACK_URL=http://example.com/api/chat-history
SESSION_HISTORY_CALLBACK_URL=http://example.com/api/session-history
PERIODIC_SYNC_CALLBACK_URL=http://example.com/api/periodic-sync

# 回调配置
CHAT_HISTORY_BATCH_SIZE=1000
SESSION_HISTORY_BATCH_SIZE=500
CHAT_HISTORY_BATCH_INTERVAL_SECONDS=300
SESSION_HISTORY_BATCH_INTERVAL_SECONDS=600

# 回调重试配置
CALLBACK_MAX_RETRIES=3
CALLBACK_RETRY_DELAY_SECONDS=5
CALLBACK_TIMEOUT_SECONDS=30

# 数据同步配置
SYNC_INTERVAL_SECONDS=300  # 定时同步间隔（秒），默认5分钟

# 普通用户发言最小间隔（秒），默认3秒
USER_MESSAGE_INTERVAL_SECS=3
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

**注意：** 创建房间时会自动触发一次数据同步和房间创建事件回调。

### 查询所有房间

```bash
curl -X GET http://localhost:3000/management/rooms \
  -H "X-Api-Key: your_secret_api_key"
```

### WebSocket 连接

```
ws://localhost:3000/ws/rooms/{room_id}?user_id={user_id}&nickname={nickname}
```

### 数据同步接口

#### 获取所有房间的同步数据（传统接口）

```bash
curl -X GET http://localhost:3000/management/sync \
  -H "X-Api-Key: your_secret_api_key"
```

#### 手动触发同步

```bash
curl -X POST http://localhost:3000/management/sync \
  -H "X-Api-Key: your_secret_api_key"
```

### 拆分后的同步接口

#### 获取房间基础信息

```bash
curl -X GET http://localhost:3000/management/sync/rooms \
  -H "X-Api-Key: your_secret_api_key"
```

#### 获取聊天记录（分页）

```bash
curl -X GET "http://localhost:3000/management/sync/chat-history/{room_id}?page=1&limit=1000&from=1640995200&to=1640998800" \
  -H "X-Api-Key: your_secret_api_key"
```

#### 获取会话历史（分页）

```bash
curl -X GET "http://localhost:3000/management/sync/session-history/{room_id}?page=1&limit=500&from=1640995200&to=1640998800" \
  -H "X-Api-Key: your_secret_api_key"
```

## 数据同步策略

系统支持多种数据同步方式：

### 1. 创建房间时同步
每次创建新房间时，系统会自动触发一次数据同步，将房间信息发送到配置的外部系统。

### 2. 定时同步
系统会根据 `SYNC_INTERVAL_SECONDS` 配置的时间间隔，定期同步所有房间的数据。

### 3. 外部系统主动拉取
外部系统可以通过 `/management/sync` GET 接口主动获取所有房间的同步数据。

### 4. 拆分式回调系统

#### 房间事件回调（实时）
- 房间创建事件
- 房间关闭事件
- 用户加入事件
- 用户离开事件

#### 聊天记录批次回调（大数据量）
- 支持分页传输
- 可配置批次大小
- 支持时间范围过滤

#### 会话历史批次回调（大数据量）
- 支持分页传输
- 可配置批次大小
- 支持时间范围过滤

#### 定时同步回调
- 房间基础信息
- 当前统计
- 数据一致性检查

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

## 回调系统配置

### 传统回调（保持向后兼容）
```bash
DATA_CALLBACK_URL=http://your-external-system.com/sync/room
```

### 拆分式回调
```bash
# 房间事件（实时）
ROOM_EVENT_CALLBACK_URL=http://your-external-system.com/api/room-events

# 聊天记录（批量）
CHAT_HISTORY_CALLBACK_URL=http://your-external-system.com/api/chat-history

# 会话历史（批量）
SESSION_HISTORY_CALLBACK_URL=http://your-external-system.com/api/session-history

# 定时同步
PERIODIC_SYNC_CALLBACK_URL=http://your-external-system.com/api/periodic-sync
```

### 回调配置选项
```bash
# 批次大小
CHAT_HISTORY_BATCH_SIZE=1000
SESSION_HISTORY_BATCH_SIZE=500

# 批次间隔
CHAT_HISTORY_BATCH_INTERVAL_SECONDS=300
SESSION_HISTORY_BATCH_INTERVAL_SECONDS=600

# 重试配置
CALLBACK_MAX_RETRIES=3
CALLBACK_RETRY_DELAY_SECONDS=5
CALLBACK_TIMEOUT_SECONDS=30
```

## WebSocket 协议说明

- 连接示例：
  ```
  ws://localhost:3000/ws/rooms/{room_id}?user_id={user_id}&nickname={nickname}
  ```
- 用户加入/发送消息时需带user_id和nickname。
- 管理员可通过高优先级通道发送自定义事件（CustomEvent），如福袋、红包。

### WsMessage 主要类型示例

```json
// 普通文本消息
{"type": "Message", "payload": {"from": "user1", "nickname": "张三", "content": "hello", "is_admin": false}}

// 用户加入
{"type": "UserJoined", "payload": {"user_id": "user2", "nickname": "李四"}}

// 用户离开
{"type": "UserLeft", "payload": {"user_id": "user2", "nickname": "李四"}}

// 当前房间人数（用于加入/离开截流推送）
{"type": "RoomStats", "payload": {"current_users": 123, "peak_users": 200}}

// 管理员自定义事件（如福袋、红包）
{"type": "CustomEvent", "payload": {"event_type": "lucky_money", "payload": {"amount": 100, "desc": "新年快乐"}}}
```

## 消息频率限制与截流

- 普通用户发言有频率限制，默认每3秒只能发一次（可通过USER_MESSAGE_INTERVAL_SECS配置）。
- 管理员不受此限制。
- 用户加入房间时，加入通知会进行截流：1秒内多次加入只推送一次，推送内容带当前房间人数。

## 高优先级/低优先级消息分片调度

- 所有消息分为高优先级（如管理员操作、系统事件、自定义事件）和低优先级（普通聊天等）。
- 低优先级消息每处理一批后主动让步，优先响应高优先级消息，保证系统灵敏度。

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
├── sync.rs         # 数据同步服务
└── callback.rs     # 回调服务
```

## 示例和测试

### 回调服务器示例
```bash
cd example/callback
python callback_server.py
```

### 测试脚本
```bash
cd example/callback
python test_callback.py
```

### 配置示例
```bash
cp example/callback/config_example.toml .env
# 编辑 .env 文件配置回调URL
```
