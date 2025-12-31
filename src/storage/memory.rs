use crate::models::LockInfo;
use crate::storage::LockStorage;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct MemoryStorage {
    locks: DashMap<String, LockInfo>, // lock_key -> LockInfo
    lock_by_id: DashMap<String, String>, // lock_id -> lock_key
    persist_path: Option<PathBuf>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            locks: DashMap::new(),
            lock_by_id: DashMap::new(),
            persist_path: None,
        }
    }

    pub fn with_persistence(persist_path: PathBuf) -> Self {
        Self {
            locks: DashMap::new(),
            lock_by_id: DashMap::new(),
            persist_path: Some(persist_path),
        }
    }

    /// 从磁盘加载数据
    pub async fn load_from_disk(&self) -> Result<usize> {
        let path = match &self.persist_path {
            Some(p) => p,
            None => return Ok(0),
        };

        if !path.exists() {
            log::info!("[PERSISTENCE] No persistence file found at {:?}", path);
            return Ok(0);
        }

        let mut file = fs::File::open(path).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;

        let data: Vec<LockInfo> = serde_json::from_str(&contents)?;
        let mut loaded_count = 0;

        for lock_info in data {
            // 只加载未过期的锁
            if !lock_info.is_expired() {
                let lock_key = lock_info.get_lock_key();
                self.lock_by_id.insert(lock_info.lock_id.clone(), lock_key.clone());
                self.locks.insert(lock_key, lock_info);
                loaded_count += 1;
            }
        }

        log::info!(
            "[PERSISTENCE] Loaded {} locks from disk (file: {:?})",
            loaded_count, path
        );
        Ok(loaded_count)
    }

    /// 持久化数据到磁盘
    pub async fn persist_to_disk(&self) -> Result<usize> {
        let path = match &self.persist_path {
            Some(p) => p,
            None => return Ok(0),
        };

        // 收集所有锁数据
        let locks: Vec<LockInfo> = self
            .locks
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        let count = locks.len();
        let json = serde_json::to_string_pretty(&locks)?;

        // 确保目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // 写入临时文件，然后重命名（原子操作）
        let temp_path = path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path).await?;
        file.write_all(json.as_bytes()).await?;
        file.sync_all().await?;
        fs::rename(temp_path, path).await?;

        log::debug!(
            "[PERSISTENCE] Persisted {} locks to disk (file: {:?})",
            count, path
        );
        Ok(count)
    }
}

#[async_trait]
impl LockStorage for MemoryStorage {
    async fn try_acquire(&self, lock_info: LockInfo) -> Result<bool> {
        let lock_key = lock_info.get_lock_key();

        // 检查是否已存在锁
        if let Some(existing_lock) = self.locks.get(&lock_key) {
            // 如果锁过期，则移除旧锁
            if existing_lock.is_expired() {
                let old_lock_id = existing_lock.lock_id.clone();
                let old_user_name = existing_lock.user_name.clone();
                let old_user_id = existing_lock.user_id.clone();
                let namespace = existing_lock.namespace.clone();
                let business_id = existing_lock.business_id.clone();
                drop(existing_lock); // 释放读锁
                self.lock_by_id.remove(&old_lock_id);
                self.locks.remove(&lock_key);
                log::info!(
                    "[EXPIRED] Lock expired and removed - lock_id: {}, namespace: {}, business_id: {}, user_id: {}, user_name: {}",
                    old_lock_id, namespace, business_id, old_user_id, old_user_name
                );
            } else if existing_lock.user_id == lock_info.user_id {
                // 同一个用户重复申请，更新心跳时间并返回现有锁ID
                let existing_lock_id = existing_lock.lock_id.clone();
                drop(existing_lock); // 释放读锁
                if let Some(mut lock) = self.locks.get_mut(&lock_key) {
                    lock.last_heartbeat = chrono::Utc::now();
                    log::info!(
                        "[REENTRANT] Same user re-acquiring lock - lock_id: {}, namespace: {}, business_id: {}, user_id: {}, user_name: {}",
                        existing_lock_id, lock.namespace, lock.business_id, lock.user_id, lock.user_name
                    );
                }
                return Ok(true);
            } else {
                // 锁仍然有效且被其他用户持有，获取失败
                return Ok(false);
            }
        }

        // 获取锁
        self.lock_by_id.insert(lock_info.lock_id.clone(), lock_key.clone());
        self.locks.insert(lock_key, lock_info);
        Ok(true)
    }

    async fn get_lock(&self, lock_key: &str) -> Result<Option<LockInfo>> {
        Ok(self.locks.get(lock_key).map(|entry| entry.value().clone()))
    }

    async fn update_heartbeat(&self, lock_id: &str) -> Result<bool> {
        let lock_key = match self.lock_by_id.get(lock_id) {
            Some(entry) => entry.value().clone(),
            None => return Ok(false),
        };

        if let Some(mut lock_info) = self.locks.get_mut(&lock_key) {
            if lock_info.lock_id == lock_id {
                lock_info.last_heartbeat = Utc::now();
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn release(&self, lock_id: &str) -> Result<bool> {
        let lock_key = match self.lock_by_id.remove(lock_id) {
            Some((_, key)) => key,
            None => return Ok(false),
        };

        if let Some((_, lock_info)) = self.locks.remove(&lock_key) {
            if lock_info.lock_id == lock_id {
                log::info!(
                    "[RELEASE] Releasing lock - lock_id: {}, namespace: {}, business_id: {}, user_id: {}, user_name: {}",
                    lock_info.lock_id, lock_info.namespace, lock_info.business_id, 
                    lock_info.user_id, lock_info.user_name
                );
                return Ok(true);
            } else {
                // 如果 lock_id 不匹配，恢复锁
                self.locks.insert(lock_key, lock_info);
            }
        }
        Ok(false)
    }

    async fn cleanup_expired(&self) -> Result<()> {
        // 收集过期的锁
        let expired: Vec<(String, String)> = self
            .locks
            .iter()
            .filter(|entry| entry.value().is_expired())
            .map(|entry| (entry.key().clone(), entry.value().lock_id.clone()))
            .collect();

        if !expired.is_empty() {
            log::info!("[CLEANUP] Found {} expired locks to clean up", expired.len());
        }

        // 删除过期的锁
        for (lock_key, lock_id) in expired {
            if let Some((_, lock_info)) = self.locks.remove(&lock_key) {
                log::info!(
                    "[EXPIRED CLEANUP] Removed expired lock - lock_id: {}, namespace: {}, business_id: {}, user_id: {}, user_name: {}, locked_at: {}",
                    lock_info.lock_id, lock_info.namespace, lock_info.business_id,
                    lock_info.user_id, lock_info.user_name, lock_info.locked_at
                );
            }
            self.lock_by_id.remove(&lock_id);
        }

        Ok(())
    }
}
