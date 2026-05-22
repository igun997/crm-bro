use actix_web::{dev::ServiceRequest, Error, HttpMessage};
use actix_web::error::ErrorUnauthorized;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};

use crate::routes::auth::Claims;

pub async fn validate_token(req: &ServiceRequest, token: &str) -> Result<Claims, Error> {
    let config = req.app_data::<actix_web::web::Data<crate::config::AppConfig>>()
        .ok_or_else(|| ErrorUnauthorized("Server config missing"))?;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|e| ErrorUnauthorized(format!("Invalid token: {}", e)))?;

    Ok(token_data.claims)
}

pub fn extract_bearer(auth_header: &str) -> Option<&str> {
    if auth_header.starts_with("Bearer ") {
        Some(&auth_header[7..])
    } else {
        None
    }
}
