#![allow(dead_code)]
use serde::Serialize;
use utoipa::ToSchema;
use actix_web::HttpResponse;

/// Standard API response wrapper
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ErrorDetail>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

/// Paginated response
#[derive(Debug, Serialize, ToSchema)]
pub struct PaginatedResponse<T: Serialize> {
    pub success: bool,
    pub data: Vec<T>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Pagination {
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
    pub total_pages: u64,
}

impl Pagination {
    pub fn new(page: u64, per_page: u64, total: u64) -> Self {
        Self {
            page,
            per_page,
            total,
            total_pages: (total + per_page - 1) / per_page,
        }
    }
}

// Builder helpers

pub fn ok<T: Serialize>(data: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(data),
        error: None,
    })
}

pub fn created<T: Serialize>(data: T) -> HttpResponse {
    HttpResponse::Created().json(ApiResponse {
        success: true,
        data: Some(data),
        error: None,
    })
}

pub fn err(status: actix_web::http::StatusCode, code: &str, message: &str) -> HttpResponse {
    HttpResponse::build(status).json(ApiResponse::<()> {
        success: false,
        data: None,
        error: Some(ErrorDetail {
            code: code.to_string(),
            message: message.to_string(),
        }),
    })
}

pub fn not_found(message: &str) -> HttpResponse {
    err(actix_web::http::StatusCode::NOT_FOUND, "NOT_FOUND", message)
}

pub fn bad_request(message: &str) -> HttpResponse {
    err(actix_web::http::StatusCode::BAD_REQUEST, "BAD_REQUEST", message)
}

pub fn unauthorized(message: &str) -> HttpResponse {
    err(actix_web::http::StatusCode::UNAUTHORIZED, "UNAUTHORIZED", message)
}

pub fn paginated<T: Serialize>(data: Vec<T>, page: u64, per_page: u64, total: u64) -> HttpResponse {
    HttpResponse::Ok().json(PaginatedResponse {
        success: true,
        data,
        pagination: Pagination::new(page, per_page, total),
    })
}
