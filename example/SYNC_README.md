# 数据同步功能说明

## 概述

Rust聊天服务器提供了完整的数据同步策略，支持多种同步方式来确保外部系统能够及时获取房间和用户数据。

## 同步策略

### 1. 创建房间时同步

**触发条件：** 每次创建新房间时自动触发

**实现方式：**
- 在 `create_room` 接口中，房间创建成功后立即调用 `SyncService::sync_room`
- 异步执行，不会阻塞房间创建流程
- 只有在配置了 `DATA_CALLBACK_URL` 时才会执行

**日志示例：**
```
INFO  chat_server::routes > 创建房间后同步成功，房间ID: 123e4567-e89b-12d3-a456-426614174000
```

### 2. 定时同步

**触发条件：** 根据 `SYNC_INTERVAL_SECONDS` 配置的时间间隔定期执行

**实现方式：**
- 服务器启动时自动启动定时同步任务
- 使用 `tokio::time::interval` 实现定时器
- 同步所有活跃房间的数据
- 并发执行，提高同步效率

**配置示例：**
```bash
# 每5分钟同步一次
SYNC_INTERVAL_SECONDS=300

# 每1小时同步一次
SYNC_INTERVAL_SECONDS=3600

# 禁用定时同步（设置为0）
SYNC_INTERVAL_SECONDS=0
```

**日志示例：**
```
INFO  chat_server::sync > 启动定时同步服务，间隔: 300秒
INFO  chat_server::sync > 完成所有房间的定时同步
```

### 3. 外部系统主动拉取

**触发条件：** 外部系统主动调用API获取数据

**API接口：**
- `GET /management/sync` - 获取所有房间的同步数据
- 需要管理员API密钥认证
- 返回完整的房间数据列表

**使用示例：**
```bash
curl -X GET http://localhost:3000/management/sync \
  -H "X-Api-Key: your_secret_api_key"
```

## 同步数据结构

同步数据包含以下信息：

```json
{
  "room_id": "123e4567-e89b-12d3-a456-426614174000",
  "room_name": "测试房间",
  "current_connections": 5,
  "admin_user_ids": ["admin1", "admin2"],
  "banned_user_ids": ["user1"],
  "created_at": 1640995200,
  "last_activity": 1640995260
}
```

## 配置选项

### 环境变量

| 变量名 | 默认值 | 说明 |
|--------|--------|------|
| `DATA_CALLBACK_URL` | 无 | 外部系统回调URL，用于推送同步数据 |
| `SYNC_INTERVAL_SECONDS` | 300 | 定时同步间隔（秒），0表示禁用 |

### 配置示例

```bash
# 基础配置
BIND_ADDRESS=0.0.0.0:3000
ADMIN_API_KEY=your_secret_api_key

# 同步配置
DATA_CALLBACK_URL=http://your-external-system.com/api/sync
SYNC_INTERVAL_SECONDS=300

# 数据库配置
DATABASE_URL=sqlite:./chat_server.db
```

## API接口

### 获取同步数据

```http
GET /management/sync
X-Api-Key: your_secret_api_key
```

**响应：**
```json
[
  {
    "room_id": "123e4567-e89b-12d3-a456-426614174000",
    "room_name": "房间1",
    "current_connections": 3,
    "admin_user_ids": ["admin1"],
    "banned_user_ids": [],
    "created_at": 1640995200,
    "last_activity": 1640995260
  }
]
```

### 手动触发同步

```http
POST /management/sync
X-Api-Key: your_secret_api_key
```

**响应：**
```http
HTTP/1.1 202 Accepted
```

## 错误处理

### 同步失败处理

1. **网络错误：** 记录错误日志，不影响其他功能
2. **认证失败：** 记录警告日志，跳过同步
3. **数据错误：** 记录错误日志，尝试继续处理其他房间

### 日志级别

- `INFO`: 同步成功、定时器启动
- `WARN`: 同步请求失败（HTTP状态码错误）
- `ERROR`: 网络错误、数据错误、任务失败

## 性能考虑

### 并发处理

- 定时同步使用并发任务处理多个房间
- 每个房间的同步独立执行，互不影响
- 使用 `tokio::spawn` 避免阻塞主线程

### 资源管理

- 同步任务使用独立的HTTP客户端
- 避免长时间占用数据库连接
- 合理的错误重试机制

## 监控和调试

### 日志监控

```bash
# 查看同步相关日志
RUST_LOG=chat_server::sync=debug cargo run

# 查看所有日志
RUST_LOG=chat_server=debug cargo run
```

### 健康检查

```bash
# 检查服务器状态
curl http://localhost:3000/management/health
```

## 最佳实践

1. **合理设置同步间隔：** 根据数据更新频率设置合适的同步间隔
2. **监控同步状态：** 定期检查日志确保同步正常
3. **错误处理：** 外部系统应该能够处理重复的同步数据
4. **数据一致性：** 使用时间戳确保数据的一致性
5. **性能优化：** 对于大量房间，考虑分批同步

## 故障排除

### 常见问题

1. **同步不工作：** 检查 `DATA_CALLBACK_URL` 配置
2. **定时同步不执行：** 检查 `SYNC_INTERVAL_SECONDS` 配置
3. **认证失败：** 检查API密钥配置
4. **网络错误：** 检查外部系统可访问性

### 调试步骤

1. 检查服务器日志
2. 验证配置参数
3. 测试外部系统连接
4. 手动触发同步测试 