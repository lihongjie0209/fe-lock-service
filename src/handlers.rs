use crate::models::{
    AcquireLockRequest, AcquireLockSuccess, ApiResponse, HeartbeatRequest,
    LockInfo, ReleaseLockRequest,
};
use crate::storage::LockStorage;
use actix_web::{web, HttpResponse};
use log::{error, info};
use std::sync::Arc;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        acquire_lock,
        heartbeat,
        release_lock
    ),
    components(
        schemas(
            AcquireLockRequest,
            AcquireLockSuccess,
            HeartbeatRequest,
            ReleaseLockRequest,
            ApiResponse<AcquireLockSuccess>,
            ApiResponse<serde_json::Value>,
        )
    ),
    tags(
        (name = "lock", description = "分布式锁接口")
    ),
    info(
        title = "分布式锁服务 API",
        version = "0.1.0",
        description = "提供分布式锁的申请、心跳和释放功能",
    )
)]
pub struct ApiDoc;

/// 申请锁接口
#[utoipa::path(
    post,
    path = "/api/lock/acquire",
    tag = "lock",
    request_body = AcquireLockRequest,
    responses(
        (status = 200, description = "申请锁成功", body = ApiResponse<AcquireLockSuccess>),
        (status = 200, description = "锁已被占用", body = ApiResponse<AcquireLockSuccess>)
    )
)]
pub async fn acquire_lock(
    storage: web::Data<Arc<dyn LockStorage>>,
    req: web::Json<AcquireLockRequest>,
) -> HttpResponse {
    info!(
        "[ACQUIRE] Attempting to acquire lock - namespace: {}, business_id: {}, user_id: {}, user_name: {}, timeout: {}s",
        req.namespace, req.business_id, req.user_id, req.user_name, req.timeout
    );

    let lock_info = LockInfo::new(&req);
    let lock_key = lock_info.get_lock_key();

    match storage.try_acquire(lock_info.clone()).await {
        Ok(acquired) => {
            if acquired {
                // 检查是否是重复申请（返回现有锁ID）
                match storage.get_lock(&lock_key).await {
                    Ok(Some(existing_lock)) => {
                        info!(
                            "[ACQUIRE SUCCESS] Lock acquired - lock_id: {}, namespace: {}, business_id: {}, user_id: {}, user_name: {}",
                            existing_lock.lock_id, existing_lock.namespace, existing_lock.business_id, 
                            existing_lock.user_id, existing_lock.user_name
                        );
                        HttpResponse::Ok().json(ApiResponse::success(AcquireLockSuccess {
                            lock_id: existing_lock.lock_id,
                        }))
                    }
                    _ => {
                        info!(
                            "[ACQUIRE SUCCESS] Lock acquired - lock_id: {}, namespace: {}, business_id: {}, user_id: {}, user_name: {}",
                            lock_info.lock_id, lock_info.namespace, lock_info.business_id, 
                            lock_info.user_id, lock_info.user_name
                        );
                        HttpResponse::Ok().json(ApiResponse::success(AcquireLockSuccess {
                            lock_id: lock_info.lock_id,
                        }))
                    }
                }
            } else {
                // 获取当前锁的持有人信息
                match storage.get_lock(&lock_key).await {
                    Ok(Some(existing_lock)) => {
                        info!(
                            "[ACQUIRE FAILED] Lock already held - namespace: {}, business_id: {}, current_holder: {} (user_id: {}), locked_at: {}, requested_by: {} (user_id: {})",
                            existing_lock.namespace, existing_lock.business_id, existing_lock.user_name, 
                            existing_lock.user_id, existing_lock.locked_at, req.user_name, req.user_id
                        );
                        HttpResponse::Ok().json(ApiResponse::<AcquireLockSuccess>::error(
                            1001,
                            format!(
                                "Lock already held by {}",
                                existing_lock.user_name
                            ),
                        ))
                    }
                    Ok(None) => {
                        error!("Lock acquisition failed but no lock info found");
                        HttpResponse::Ok().json(ApiResponse::<AcquireLockSuccess>::error(
                            1002,
                            "Lock acquisition failed".to_string(),
                        ))
                    }
                    Err(e) => {
                        error!("Failed to get lock info: {}", e);
                        HttpResponse::Ok().json(ApiResponse::<AcquireLockSuccess>::error(
                            1003,
                            format!("Failed to get lock info: {}", e),
                        ))
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to acquire lock: {}", e);
            HttpResponse::Ok().json(ApiResponse::<AcquireLockSuccess>::error(
                1004,
                format!("Failed to acquire lock: {}", e),
            ))
        }
    }
}

/// 心跳接口
#[utoipa::path(
    post,
    path = "/api/lock/heartbeat",
    tag = "lock",
    request_body = HeartbeatRequest,
    responses(
        (status = 200, description = "心跳成功", body = ApiResponse<serde_json::Value>),
        (status = 200, description = "锁不存在或已过期", body = ApiResponse<serde_json::Value>)
    )
)]
pub async fn heartbeat(
    storage: web::Data<Arc<dyn LockStorage>>,
    req: web::Json<HeartbeatRequest>,
) -> HttpResponse {
    info!("Heartbeat request: lock_id={}", req.lock_id);

    match storage.update_heartbeat(&req.lock_id).await {
        Ok(updated) => {
            if updated {
                info!("Heartbeat updated successfully: {}", req.lock_id);
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "updated": true
                })))
            } else {
                info!("Lock not found or expired: {}", req.lock_id);
                HttpResponse::Ok().json(ApiResponse::<serde_json::Value>::error(
                    2001,
                    "Lock not found or expired".to_string(),
                ))
            }
        }
        Err(e) => {
            error!("Failed to update heartbeat: {}", e);
            HttpResponse::Ok().json(ApiResponse::<serde_json::Value>::error(
                2002,
                format!("Failed to update heartbeat: {}", e),
            ))
        }
    }
}

/// 释放锁接口
#[utoipa::path(
    post,
    path = "/api/lock/release",
    tag = "lock",
    request_body = ReleaseLockRequest,
    responses(
        (status = 200, description = "释放锁成功", body = ApiResponse<serde_json::Value>),
        (status = 200, description = "锁不存在或不属于当前用户", body = ApiResponse<serde_json::Value>)
    )
)]
pub async fn release_lock(
    storage: web::Data<Arc<dyn LockStorage>>,
    req: web::Json<ReleaseLockRequest>,
) -> HttpResponse {
    info!("[RELEASE] Attempting to release lock - lock_id: {}", req.lock_id);

    match storage.release(&req.lock_id).await {
        Ok(released) => {
            if released {
                info!("[RELEASE SUCCESS] Lock released - lock_id: {}", req.lock_id);
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "released": true
                })))
            } else {
                info!("[RELEASE FAILED] Lock not found or not owned - lock_id: {}", req.lock_id);
                HttpResponse::Ok().json(ApiResponse::<serde_json::Value>::error(
                    3001,
                    "Lock not found or not owned".to_string(),
                ))
            }
        }
        Err(e) => {
            error!("Failed to release lock: {}", e);
            HttpResponse::Ok().json(ApiResponse::<serde_json::Value>::error(
                3002,
                format!("Failed to release lock: {}", e),
            ))
        }
    }
}
