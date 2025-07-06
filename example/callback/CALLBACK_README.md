# 房间信息同步回调系统

这是一个完整的房间信息同步回调系统示例，用于在房间关闭时将房间数据同步到外部系统。

## 系统架构

```
┌─────────────────┐    HTTP POST    ┌──────────────────┐
│   Rust服务器     │ ──────────────► │  回调服务器       │
│  (WebSocket)    │                 │  (Flask)         │
└─────────────────┘                 └──────────────────┘
         │                                   │
         │ 房间关闭时                         │ 数据存储
         ▼                                   ▼
┌─────────────────┐                 ┌──────────────────┐
│   数据库        │                 │   SQLite数据库   │
│  (房间数据)     │                 │  (同步数据)      │
└─────────────────┘                 └──────────────────┘
```

## 功能特性

### Rust WebSocket服务器
- 实时WebSocket通信
- 房间管理（创建、关闭、管理员设置）
- 用户会话跟踪
- 聊天消息记录
- 房间关闭时自动数据同步

### 回调服务器 (Python Flask)
- 接收房间数据同步
- 数据持久化存储
- RESTful API接口
- 统计信息查询
- 健康检查

## 快速开始

### 1. 安装依赖

#### Python依赖
```bash
pip install flask requests
```

#### Rust依赖
确保你的`Cargo.toml`包含以下依赖：
```toml
[dependencies]
reqwest = { version = "0.11", features = ["json"] }
tokio = { version = "1.0", features = ["full"] }
```

### 2. 配置Rust服务器

复制配置文件示例：
```bash
cp config_example.toml config.toml
```

编辑`config.toml`，设置回调URL：
```toml
[callback]
data_callback_url = "http://localhost:8080/sync/room"
```

### 3. 启动回调服务器

```bash
python callback_server.py
```

服务器将在 `http://localhost:8080` 启动。

### 4. 启动Rust服务器

```bash
cargo run
```

服务器将在 `http://localhost:3000` 启动。

### 5. 测试系统

```bash
python test_callback.py
```

## API接口

### 回调服务器API

#### 健康检查
```http
GET /health
```

#### 接收房间数据同步
```http
POST /sync/room
Content-Type: application/json

{
  "room_id": "550e8400-e29b-41d4-a716-446655440000",
  "admin_user_ids": ["admin1", "admin2"],
  "start_time": 1640995200,
  "stats": {
    "current_users": 5,
    "peak_users": 15,
    "total_joins": 25
  },
  "chat_history": [...],
  "session_history": [...]
}
```

#### 获取房间列表
```http
GET /rooms
```

#### 获取房间详情
```http
GET /rooms/{room_id}
```

#### 获取统计信息
```http
GET /stats
```

### Rust服务器API

#### 创建房间
```http
POST /rooms
X-Api-Key: your-secret-admin-key-here
Content-Type: application/json

{
  "room_name": "测试房间",
  "admin_user_ids": ["admin1", "admin2"]
}
```

#### 关闭房间（触发数据同步）
```http
DELETE /rooms/{room_id}
X-Api-Key: your-secret-admin-key-here
```

## 数据同步流程

1. **房间创建**: 用户通过API创建房间
2. **实时通信**: 用户通过WebSocket进行实时通信
3. **数据记录**: 系统记录聊天消息、用户会话等信息
4. **房间关闭**: 管理员或系统关闭房间
5. **数据同步**: 自动向回调URL发送完整的房间数据
6. **数据存储**: 回调服务器将数据存储到本地数据库

## 数据结构

### DataSyncPayload
```rust
pub struct DataSyncPayload {
    pub room_id: Uuid,
    pub admin_user_ids: HashSet<String>,
    pub start_time: i64,
    pub stats: RoomStats,
    pub chat_history: Vec<ChatHistoryEntry>,
    pub session_history: Vec<SessionHistoryEntry>,
}
```

### 数据库表结构

#### room_syncs (房间同步记录)
- `id`: 主键
- `room_id`: 房间ID
- `sync_time`: 同步时间
- `admin_user_ids`: 管理员用户ID列表
- `start_time`: 房间开始时间
- `current_users`: 当前用户数
- `peak_users`: 峰值用户数
- `total_joins`: 总加入次数
- `chat_count`: 聊天消息数
- `session_count`: 会话记录数
- `raw_data`: 原始数据

#### chat_records (聊天记录)
- `id`: 主键
- `room_id`: 房间ID
- `user_id`: 用户ID
- `content`: 消息内容
- `created_at`: 创建时间
- `sync_time`: 同步时间

#### session_records (会话记录)
- `id`: 主键
- `room_id`: 房间ID
- `user_id`: 用户ID
- `join_time`: 加入时间
- `leave_time`: 离开时间
- `duration_seconds`: 持续时间
- `sync_time`: 同步时间

## 配置选项

### 回调服务器配置
- `timeout_seconds`: 回调超时时间（默认30秒）
- `max_retries`: 最大重试次数（默认3次）
- `retry_delay_seconds`: 重试延迟（默认5秒）

### 安全配置
- 支持API密钥认证
- 支持HTTPS回调URL
- 可配置回调认证头

## 错误处理

### 回调失败处理
- 自动重试机制
- 错误日志记录
- 超时处理
- 网络异常处理

### 数据完整性
- 事务处理确保数据一致性
- 重复数据检测
- 数据验证

## 监控和日志

### 日志记录
- 回调请求日志
- 错误日志
- 性能监控日志

### 健康检查
- 服务器状态监控
- 数据库连接检查
- 回调URL可用性检查

## 扩展功能

### 支持的回调类型
- 房间关闭同步
- 定期数据同步
- 实时事件推送

### 数据格式支持
- JSON格式
- 可扩展的自定义格式

### 存储后端
- SQLite（当前实现）
- PostgreSQL
- MySQL
- MongoDB

## 故障排除

### 常见问题

1. **回调服务器无法启动**
   - 检查端口8080是否被占用
   - 确保Python依赖已安装

2. **数据同步失败**
   - 检查网络连接
   - 验证回调URL配置
   - 查看错误日志

3. **数据库错误**
   - 检查数据库文件权限
   - 确保磁盘空间充足

### 调试模式

启动回调服务器时启用调试模式：
```bash
FLASK_ENV=development python callback_server.py
```

## 性能优化

### 批量处理
- 批量数据库写入
- 异步回调处理
- 连接池管理

### 缓存策略
- 内存缓存
- Redis缓存
- 数据压缩

## 安全考虑

### 数据安全
- HTTPS传输
- API密钥认证
- 数据加密

### 访问控制
- IP白名单
- 请求频率限制
- 数据访问权限控制

## 部署建议

### 生产环境
- 使用反向代理（Nginx）
- 配置SSL证书
- 设置监控告警
- 数据备份策略

### 容器化部署
- Docker镜像构建
- Kubernetes部署
- 服务发现配置

## 贡献指南

1. Fork项目
2. 创建功能分支
3. 提交更改
4. 创建Pull Request

## 许可证

MIT License 