use actix_web::{dev::ServiceRequest, Error};
use actix_web::error::ErrorUnauthorized;

use crate::auth::jwt::Claims;

#[allow(dead_code)]
pub async fn validate_token(req: &ServiceRequest, token: &str) -> Result<Claims, Error> {
    let config = req.app_data::<actix_web::web::Data<crate::config::AppConfig>>()
        .ok_or_else(|| ErrorUnauthorized("Server config missing"))?;

    let token_data = crate::auth::jwt::decode_jwt(token, &config.jwt_secret)
        .map_err(|e| ErrorUnauthorized(format!("Invalid token: {}", e)))?;

    Ok(token_data.claims)
}

#[allow(dead_code)]
pub fn extract_bearer(auth_header: &str) -> Option<&str> {
    if auth_header.starts_with("Bearer ") {
        Some(&auth_header[7..])
    } else {
        None
    }
}
