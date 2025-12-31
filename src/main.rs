mod config;
mod handlers;
mod models;
mod storage;

use actix_web::{middleware::Logger, web, App, HttpServer};
use config::{Config, StorageType};
use log::info;
use std::sync::Arc;
use std::time::Duration;
use storage::memory::MemoryStorage;
use storage::redis::RedisStorage;
use storage::LockStorage;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // 加载配置
    let config = Config::from_env();
    info!("Starting fe-lock-service with config: {:?}", config);

    // 创建存储
    let (storage, memory_storage_for_persist): (Arc<dyn LockStorage>, Option<Arc<MemoryStorage>>) = match config.storage_type {
        StorageType::Memory => {
            info!("Using memory storage");
            
            let memory_storage = if config.memory_persist_enabled {
                info!("Memory persistence enabled: {}", config.memory_persist_path);
                info!("Persistence interval: {} seconds", config.memory_persist_interval);
                Arc::new(MemoryStorage::with_persistence(
                    std::path::PathBuf::from(&config.memory_persist_path)
                ))
            } else {
                info!("Memory persistence disabled");
                Arc::new(MemoryStorage::new())
            };
            
            // 尝试从磁盘加载数据
            if config.memory_persist_enabled {
                match memory_storage.load_from_disk().await {
                    Ok(count) => {
                        if count > 0 {
                            info!("Successfully restored {} locks from disk", count);
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to load from disk: {}", e);
                    }
                }
            }
            
            let persist_ref = if config.memory_persist_enabled {
                Some(memory_storage.clone())
            } else {
                None
            };
            
            (memory_storage as Arc<dyn LockStorage>, persist_ref)
        }
        StorageType::Redis => {
            info!("Using Redis storage");
            let redis_url = config.redis_url.as_ref().expect("Redis URL not configured");
            let redis_storage = RedisStorage::new(
                redis_url,
                config.redis_username.clone(),
                config.redis_password.clone(),
                config.redis_db,
            )
            .await
            .expect("Failed to connect to Redis");
            (Arc::new(redis_storage) as Arc<dyn LockStorage>, None)
        }
    };

    // 启动清理任务（仅内存存储需要）
    if config.storage_type == StorageType::Memory {
        let storage_clone = storage.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                if let Err(e) = storage_clone.cleanup_expired().await {
                    log::error!("Failed to cleanup expired locks: {}", e);
                }
            }
        });
        
        // 启动持久化任务
        if let Some(memory_storage) = memory_storage_for_persist {
            let persist_interval = config.memory_persist_interval;
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(persist_interval));
                loop {
                    interval.tick().await;
                    
                    if let Err(e) = memory_storage.persist_to_disk().await {
                        log::error!("[PERSISTENCE] Failed to persist to disk: {}", e);
                    }
                }
            });
        }
    }

    let bind_addr = format!("{}:{}", config.server_host, config.server_port);
    info!("Server starting on http://{}", bind_addr);
    info!("Swagger UI available at http://{}/swagger-ui/", bind_addr);

    // 启动 HTTP 服务
    HttpServer::new(move || {
        let openapi = handlers::ApiDoc::openapi();
        
        App::new()
            .wrap(Logger::default())
            .app_data(web::Data::new(storage.clone()))
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", openapi.clone())
            )
            .route("/api/lock/acquire", web::post().to(handlers::acquire_lock))
            .route("/api/lock/heartbeat", web::post().to(handlers::heartbeat))
            .route("/api/lock/release", web::post().to(handlers::release_lock))
    })
    .bind(&bind_addr)?
    .run()
    .await
}
