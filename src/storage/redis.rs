use crate::models::LockInfo;
use crate::storage::LockStorage;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, RedisError};
use std::str::FromStr;

pub struct RedisStorage {
    client: ConnectionManager,
    prefix: String,
}

impl RedisStorage {
    pub async fn new(
        redis_url: &str,
        username: Option<String>,
        password: Option<String>,
        db: Option<i64>,
    ) -> Result<Self> {
        // 构建连接信息
        let mut connection_info = redis::ConnectionInfo::from_str(redis_url)?;
        
        // 设置认证信息
        if let Some(pwd) = password {
            connection_info.redis.password = Some(pwd);
        }
        if let Some(user) = username {
            connection_info.redis.username = Some(user);
        }
        if let Some(database) = db {
            connection_info.redis.db = database;
        }

        let client = redis::Client::open(connection_info)?;
        let connection = ConnectionManager::new(client).await?;
        Ok(Self {
            client: connection,
            prefix: "lock:".to_string(),
        })
    }

    fn get_lock_key(&self, lock_key: &str) -> String {
        format!("{}data:{}", self.prefix, lock_key)
    }

    fn get_lock_id_key(&self, lock_id: &str) -> String {
        format!("{}id:{}", self.prefix, lock_id)
    }
}

#[async_trait]
impl LockStorage for RedisStorage {
    async fn try_acquire(&self, lock_info: LockInfo) -> Result<bool> {
        let lock_key = self.get_lock_key(&lock_info.get_lock_key());
        let lock_id_key = self.get_lock_id_key(&lock_info.lock_id);
        let mut conn = self.client.clone();

        // 检查锁是否存在
        let existing: Option<String> = conn.get(&lock_key).await?;
        if let Some(existing_data) = existing {
            // 解析现有锁信息
            if let Ok(existing_lock) = serde_json::from_str::<LockInfo>(&existing_data) {
                if existing_lock.is_expired() {
                    // 锁已过期，删除旧锁
                    log::info!(
                        "[EXPIRED] Lock expired - lock_id: {}, namespace: {}, business_id: {}, user_id: {}, user_name: {}",
                        existing_lock.lock_id, existing_lock.namespace, existing_lock.business_id,
                        existing_lock.user_id, existing_lock.user_name
                    );
                    let old_lock_id_key = self.get_lock_id_key(&existing_lock.lock_id);
                    let _: Result<(), RedisError> = conn.del(&old_lock_id_key).await;
                } else if existing_lock.user_id == lock_info.user_id {
                    // 同一个用户重复申请，更新心跳时间
                    log::info!(
                        "[REENTRANT] Same user re-acquiring lock - lock_id: {}, namespace: {}, business_id: {}, user_id: {}, user_name: {}",
                        existing_lock.lock_id, existing_lock.namespace, existing_lock.business_id,
                        existing_lock.user_id, existing_lock.user_name
                    );
                    let mut updated_lock = existing_lock;
                    updated_lock.last_heartbeat = Utc::now();
                    let lock_data = serde_json::to_string(&updated_lock)?;
                    let ttl = updated_lock.timeout as u64;
                    let _: () = conn.set_ex(&lock_key, &lock_data, ttl).await?;
                    return Ok(true);
                } else {
                    // 锁被其他用户持有
                    return Ok(false);
                }
            }
        }

        // 设置锁
        let lock_data = serde_json::to_string(&lock_info)?;
        let ttl = lock_info.timeout as usize;

        // 使用 SET NX 确保原子性
        let result: bool = conn
            .set_nx(&lock_key, &lock_data)
            .await?;

        if result {
            // 设置过期时间
            let _: () = conn.expire(&lock_key, ttl as i64).await?;
            // 保存 lock_id -> lock_key 映射
            let _: () = conn
                .set_ex(&lock_id_key, lock_info.get_lock_key(), ttl as u64)
                .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn get_lock(&self, lock_key: &str) -> Result<Option<LockInfo>> {
        let key = self.get_lock_key(lock_key);
        let mut conn = self.client.clone();
        let data: Option<String> = conn.get(&key).await?;

        match data {
            Some(json_str) => {
                let lock_info: LockInfo = serde_json::from_str(&json_str)?;
                Ok(Some(lock_info))
            }
            None => Ok(None),
        }
    }

    async fn update_heartbeat(&self, lock_id: &str) -> Result<bool> {
        let lock_id_key = self.get_lock_id_key(lock_id);
        let mut conn = self.client.clone();

        // 获取 lock_key
        let lock_key: Option<String> = conn.get(&lock_id_key).await?;
        let lock_key = match lock_key {
            Some(key) => key,
            None => return Ok(false),
        };

        let full_lock_key = self.get_lock_key(&lock_key);

        // 获取锁信息
        let data: Option<String> = conn.get(&full_lock_key).await?;
        let data = match data {
            Some(d) => d,
            None => return Ok(false),
        };

        let mut lock_info: LockInfo = serde_json::from_str(&data)?;
        if lock_info.lock_id != lock_id {
            return Ok(false);
        }

        // 更新心跳时间
        lock_info.last_heartbeat = Utc::now();
        let lock_data = serde_json::to_string(&lock_info)?;
        let ttl = lock_info.timeout as usize;

        // 更新锁数据和过期时间
        let _: () = conn.set_ex(&full_lock_key, &lock_data, ttl as u64).await?;
        let _: () = conn.expire(&lock_id_key, ttl as i64).await?;

        Ok(true)
    }

    async fn release(&self, lock_id: &str) -> Result<bool> {
        let lock_id_key = self.get_lock_id_key(lock_id);
        let mut conn = self.client.clone();

        // 获取 lock_key
        let lock_key: Option<String> = conn.get(&lock_id_key).await?;
        let lock_key = match lock_key {
            Some(key) => key,
            None => return Ok(false),
        };

        let full_lock_key = self.get_lock_key(&lock_key);

        // 验证锁所有权
        let data: Option<String> = conn.get(&full_lock_key).await?;
        if let Some(data) = data {
            let lock_info: LockInfo = serde_json::from_str(&data)?;
            if lock_info.lock_id != lock_id {
                return Ok(false);
            }
            
            log::info!(
                "[RELEASE] Releasing lock - lock_id: {}, namespace: {}, business_id: {}, user_id: {}, user_name: {}",
                lock_info.lock_id, lock_info.namespace, lock_info.business_id,
                lock_info.user_id, lock_info.user_name
            );
        } else {
            return Ok(false);
        }

        // 删除锁
        let _: () = conn.del(&full_lock_key).await?;
        let _: () = conn.del(&lock_id_key).await?;

        Ok(true)
    }

    async fn cleanup_expired(&self) -> Result<()> {
        // Redis 会自动清理过期的键，无需手动清理
        Ok(())
    }
}
