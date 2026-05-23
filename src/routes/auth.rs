use actix_web::{web, HttpResponse, post};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::jwt::{build_claims, encode_jwt};
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
    pub expires_in: u64,
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
    // TODO: validate against DB users table (Task 5)
    // Placeholder: accept test@test.com / password
    if body.email != "test@test.com" || body.password != "password" {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Invalid credentials"
        }));
    }

    let expires_in: u64 = 3600;
    // Placeholder user id 1 until Task 5 wires up real DB lookup
    let claims = build_claims(1, None, false, expires_in);

    let token = encode_jwt(&claims, &config.jwt_secret)
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
