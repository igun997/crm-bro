use actix_web::{get, web, HttpResponse};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub database: String,
}

/// Health check - verifies DB connection
#[utoipa::path(
    get,
    path = "/api/health",
    responses(
        (status = 200, description = "Service healthy", body = HealthResponse),
        (status = 503, description = "Service unhealthy", body = HealthResponse),
    ),
    tag = "Health"
)]
#[get("/health")]
pub async fn health_check(db: web::Data<DatabaseConnection>) -> HttpResponse {
    let db_status = match sea_orm::DbConn::ping(db.get_ref()).await {
        Ok(_) => "connected".to_string(),
        Err(e) => format!("error: {}", e),
    };

    let healthy = db_status == "connected";
    let resp = HealthResponse {
        status: if healthy {
            "ok".into()
        } else {
            "degraded".into()
        },
        database: db_status,
    };

    if healthy {
        HttpResponse::Ok().json(resp)
    } else {
        HttpResponse::ServiceUnavailable().json(resp)
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health_check);
}
