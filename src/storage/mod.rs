pub mod memory;
pub mod redis;

use crate::models::LockInfo;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait LockStorage: Send + Sync {
    /// 尝试获取锁
    async fn try_acquire(&self, lock_info: LockInfo) -> Result<bool>;

    /// 获取锁信息
    async fn get_lock(&self, lock_key: &str) -> Result<Option<LockInfo>>;

    /// 更新心跳
    async fn update_heartbeat(&self, lock_id: &str) -> Result<bool>;

    /// 释放锁
    async fn release(&self, lock_id: &str) -> Result<bool>;

    /// 清理过期锁
    async fn cleanup_expired(&self) -> Result<()>;
}
