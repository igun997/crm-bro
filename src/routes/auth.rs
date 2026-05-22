use actix_web::{web, HttpResponse, post};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use jsonwebtoken::{encode, Header, EncodingKey};
use chrono::{Utc, Duration};

use crate::config::AppConfig;

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub token: String,
    pub token_type: String,
    pub expires_in: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
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
) -> HttpResponse {
    // TODO: validate against DB users table
    // Placeholder: accept test@test.com / password
    if body.email != "test@test.com" || body.password != "password" {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Invalid credentials"
        }));
    }

    let now = Utc::now();
    let expires_in = 3600i64; // 1 hour
    let claims = Claims {
        sub: body.email.clone(),
        iat: now.timestamp() as usize,
        exp: (now + Duration::seconds(expires_in)).timestamp() as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .expect("JWT encoding failed");

    HttpResponse::Ok().json(LoginResponse {
        token,
        token_type: "Bearer".into(),
        expires_in,
    })
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(login);
}
