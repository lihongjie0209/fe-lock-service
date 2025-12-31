# Fe Lock Service

一个使用 Rust 编写的分布式锁服务，为前端提供分布式锁能力。

## 功能特性

- 🔒 **三个核心接口**：申请锁、心跳续期、释放锁
- 💾 **双存储支持**：Redis 或本地内存
- ⏰ **自动超时释放**：支持设置超时时间
- 🔄 **心跳机制**：保持锁的活跃状态
- 📊 **统一响应格式**：符合标准的 API 响应结构

## 接口说明

### 1. 申请锁 `/api/lock/acquire`

**请求参数：**
```json
{
  "namespace": "order",
  "user_id": "user123",
  "user_name": "张三",
  "business_id": "order_001",
  "timeout": 60
}
```

**成功响应：**
```json
{
  "code": 0,
  "message": "success",
  "data": {
    "lock_id": "550e8400-e29b-41d4-a716-446655440000"
  },
  "success": true
}
```

**失败响应（锁已被占用）：**
```json
{
  "code": 1001,
  "message": "Lock already held by 李四",
  "data": null,
  "success": false
}
```

### 2. 心跳 `/api/lock/heartbeat`

**请求参数：**
```json
{
  "lock_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**响应：**
```json
{
  "code": 0,
  "message": "success",
  "data": {
    "updated": true
  },
  "success": true
}
```

### 3. 释放锁 `/api/lock/release`

**请求参数：**
```json
{
  "lock_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**响应：**
```json
{
  "code": 0,
  "message": "success",
  "data": {
    "released": true
  },
  "success": true
}
```

## 环境配置

通过环境变量配置服务：

```bash
# 存储类型：memory 或 redis（默认：memory）
STORAGE_TYPE=memory

# Redis 配置（仅当 STORAGE_TYPE=redis 时需要）
REDIS_URL=redis://127.0.0.1:6379
REDIS_USERNAME=your_username    # 可选
REDIS_PASSWORD=your_password    # 可选
REDIS_DB=0                      # 可选，默认为 0

# 服务器配置
SERVER_HOST=127.0.0.1
SERVER_PORT=8080
```

## 快速开始

### 使用内存存储

```bash
# 设置环境变量
$env:STORAGE_TYPE="memory"
$env:SERVER_PORT="8080"

# 运行服务
cargo run
```

### 使用 Redis 存储

```bash
# 启动 Redis
docker run -d -p 6379:6379 redis:latest

# 设置环境变量
$env:STORAGE_TYPE="redis"
$env:REDIS_URL="redis://127.0.0.1:6379"
$env:REDIS_PASSWORD="your_password"  # 如果需要
$env:REDIS_DB="0"                     # 可选
$env:SERVER_PORT="8080"

# 运行服务
cargo run
```

## 构建

```bash
# 开发构建
cargo build

# 发布构建
cargo build --release
```

## 测试示例

### 申请锁
```bash
curl -X POST http://localhost:8080/api/lock/acquire `
  -H "Content-Type: application/json" `
  -d '{
    "namespace": "order",
    "user_id": "user123",
    "user_name": "张三",
    "business_id": "order_001",
    "timeout": 60
  }'
```

### 心跳
```bash
curl -X POST http://localhost:8080/api/lock/heartbeat `
  -H "Content-Type: application/json" `
  -d '{
    "lock_id": "your-lock-id-here"
  }'
```

### 释放锁
```bash
curl -X POST http://localhost:8080/api/lock/release `
  -H "Content-Type: application/json" `
  -d '{
    "lock_id": "your-lock-id-here"
  }'
```

## 项目结构

```
src/
├── main.rs           # 主程序入口
├── config.rs         # 配置管理
├── models.rs         # 数据模型定义
├── handlers.rs       # HTTP 处理器
└── storage/          # 存储层
    ├── mod.rs        # 存储接口定义
    ├── memory.rs     # 内存存储实现
    └── redis.rs      # Redis 存储实现
```

## 技术栈

- **Web 框架**: Actix-Web 4.5
- **异步运行时**: Tokio
- **Redis 客户端**: redis-rs
- **序列化**: Serde
- **日志**: log + env_logger

## 协议标准

- 统一使用 POST 方法
- 请求和响应均为 JSON 格式
- HTTP 状态码始终返回 200
- 业务状态通过响应体中的 `code` 和 `success` 字段表示

## License

MIT
