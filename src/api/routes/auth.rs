use actix_web::{post, web, HttpResponse};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::jwt::{build_claims, encode_jwt};
use crate::auth::password::verify_password;
use crate::infrastructure::config::AppConfig;
use crate::models::user;

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"email": "admin@acme.com", "password": "s3cret"}))]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"id": 1, "tenant_id": 1, "name": "Admin", "email": "admin@acme.com", "is_superadmin": false}))]
pub struct LoginUser {
    pub id: i32,
    pub tenant_id: Option<i32>,
    pub name: String,
    pub email: String,
    pub is_superadmin: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "success": true,
    "token": "eyJhbGciOiJIUzI1NiIs...",
    "token_type": "Bearer",
    "expires_in": 86400,
    "user": {"id": 1, "tenant_id": 1, "name": "Admin", "email": "admin@acme.com", "is_superadmin": false},
    "error": null
}))]
pub struct LoginResponse {
    pub success: bool,
    pub token: Option<String>,
    pub token_type: Option<String>,
    pub expires_in: Option<u64>,
    pub user: Option<LoginUser>,
    pub error: Option<String>,
}

/// Login - returns JWT token
#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials"),
    ),
    tag = "Auth"
)]
#[post("/auth/login")]
pub async fn login(
    body: web::Json<LoginRequest>,
    config: web::Data<AppConfig>,
    db: web::Data<DatabaseConnection>,
) -> HttpResponse {
    let user = match user::Entity::find()
        .filter(user::Column::Email.eq(&body.email))
        .one(db.get_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => return invalid_credentials(),
        Err(error) => {
            tracing::error!(%error, "User lookup failed during login");
            return HttpResponse::InternalServerError().json(LoginResponse {
                success: false,
                token: None,
                token_type: None,
                expires_in: None,
                user: None,
                error: Some("Login failed".to_string()),
            });
        }
    };

    if !user.is_active || !verify_password(&body.password, &user.password_hash) {
        return invalid_credentials();
    }

    let expires_in: u64 = 3600;
    let claims = build_claims(user.id, user.tenant_id, user.is_superadmin, expires_in);

    let token = match encode_jwt(&claims, &config.jwt_secret) {
        Ok(token) => token,
        Err(error) => {
            tracing::error!(%error, "JWT encoding failed");
            return HttpResponse::InternalServerError().json(LoginResponse {
                success: false,
                token: None,
                token_type: None,
                expires_in: None,
                user: None,
                error: Some("Token creation failed".to_string()),
            });
        }
    };

    HttpResponse::Ok().json(LoginResponse {
        success: true,
        token: Some(token),
        token_type: Some("Bearer".into()),
        expires_in: Some(expires_in),
        user: Some(LoginUser {
            id: user.id,
            tenant_id: user.tenant_id,
            name: user.name,
            email: user.email,
            is_superadmin: user.is_superadmin,
        }),
        error: None,
    })
}

fn invalid_credentials() -> HttpResponse {
    HttpResponse::Unauthorized().json(LoginResponse {
        success: false,
        token: None,
        token_type: None,
        expires_in: None,
        user: None,
        error: Some("Invalid credentials".to_string()),
    })
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(login);
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http::StatusCode, test, App};
    use sea_orm::Database;

    async fn login_status(email: &str, password: &str) -> StatusCode {
        let _ = dotenvy::dotenv();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL for auth route test");
        let db = Database::connect(&database_url).await.expect("db connect");
        let config = AppConfig {
            database_url,
            jwt_secret: "test-auth-secret".to_string(),
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            app_base_url: "http://localhost:8080".into(),
            storage_backend: "local".to_string(),
            storage_local_dir: "media".to_string(),
            r2_endpoint: None,
            r2_access_key_id: None,
            r2_secret_access_key: None,
            r2_bucket: None,
            r2_public_base_url: None,
        };

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(db))
                .app_data(web::Data::new(config))
                .configure(configure),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/auth/login")
            .set_json(serde_json::json!({
                "email": email,
                "password": password
            }))
            .to_request();

        test::call_service(&app, req).await.status()
    }

    fn test_admin_credentials() -> Option<(String, String)> {
        Some((
            std::env::var("CRM_BRO_TEST_ADMIN_EMAIL").ok()?,
            std::env::var("CRM_BRO_TEST_ADMIN_PASSWORD").ok()?,
        ))
    }

    #[actix_rt::test]
    async fn seeded_admin_can_login_with_real_db_user() {
        let Some((email, password)) = test_admin_credentials() else {
            return;
        };
        assert_eq!(login_status(&email, &password).await, StatusCode::OK);
    }

    #[actix_rt::test]
    async fn seeded_admin_wrong_password_is_rejected() {
        let Some((email, _)) = test_admin_credentials() else {
            return;
        };
        assert_eq!(
            login_status(&email, "wrong-password").await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[actix_rt::test]
    async fn old_stub_credentials_are_rejected() {
        assert_eq!(
            login_status("test@test.com", "password").await,
            StatusCode::UNAUTHORIZED
        );
    }
}
