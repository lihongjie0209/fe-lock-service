use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

fn default_namespace() -> String {
    "default".to_string()
}

/// 申请锁请求
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct AcquireLockRequest {
    #[serde(default = "default_namespace")]
    #[schema(example = "default")]
    pub namespace: String,
    #[schema(example = "user123")]
    pub user_id: String,
    #[schema(example = "张三")]
    pub user_name: String,
    #[schema(example = "order_001")]
    pub business_id: String,
    #[schema(example = 60)]
    pub timeout: u64, // 超时时间（秒）
}

/// 申请锁成功响应
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct AcquireLockSuccess {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub lock_id: String,
}

/// 申请锁失败响应
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct AcquireLockFailure {
    pub current_holder: String,
    pub locked_at: DateTime<Utc>,
}

/// 心跳请求
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct HeartbeatRequest {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub lock_id: String,
}

/// 释放锁请求
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ReleaseLockRequest {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub lock_id: String,
}

/// 锁信息
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LockInfo {
    pub lock_id: String,
    pub namespace: String,
    pub user_id: String,
    pub user_name: String,
    pub business_id: String,
    pub timeout: u64,
    pub locked_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
}

impl LockInfo {
    pub fn new(request: &AcquireLockRequest) -> Self {
        let now = Utc::now();
        Self {
            lock_id: Uuid::new_v4().to_string(),
            namespace: request.namespace.clone(),
            user_id: request.user_id.clone(),
            user_name: request.user_name.clone(),
            business_id: request.business_id.clone(),
            timeout: request.timeout,
            locked_at: now,
            last_heartbeat: now,
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(self.last_heartbeat);
        elapsed.num_seconds() as u64 >= self.timeout
    }

    pub fn get_lock_key(&self) -> String {
        format!("{}:{}", self.namespace, self.business_id)
    }
}

/// 统一响应结构
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: Option<T>,
    pub success: bool,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            message: "success".to_string(),
            data: Some(data),
            success: true,
        }
    }

    pub fn error(code: i32, message: String) -> Self {
        Self {
            code,
            message,
            data: None,
            success: false,
        }
    }
}
