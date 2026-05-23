use actix_web::{post, web, HttpResponse};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::password::hash_password;
use crate::auth::CurrentUser;
use crate::models::{role, tenant, user, user_role};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTenantRequest {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TenantResponse {
    pub id: i32,
    pub name: String,
    pub slug: String,
    pub is_active: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTenantUserRequest {
    pub email: String,
    pub password: String,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminUserResponse {
    pub id: i32,
    pub email: String,
    pub name: String,
    pub tenant_id: Option<i32>,
    pub is_superadmin: bool,
    pub is_active: bool,
}

#[utoipa::path(
    post,
    path = "/api/admin/tenants",
    request_body = CreateTenantRequest,
    responses(
        (status = 200, description = "Tenant created", body = TenantResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    tag = "Admin"
)]
#[post("/admin/tenants")]
pub async fn create_tenant(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    body: web::Json<CreateTenantRequest>,
) -> HttpResponse {
    if !current.0.is_superadmin {
        return forbidden();
    }

    let new_tenant = tenant::ActiveModel {
        name: Set(body.name.clone()),
        slug: Set(body.slug.clone()),
        is_active: Set(true),
        ..Default::default()
    };

    match new_tenant.insert(db.get_ref()).await {
        Ok(tenant) => HttpResponse::Ok().json(TenantResponse {
            id: tenant.id,
            name: tenant.name,
            slug: tenant.slug,
            is_active: tenant.is_active,
        }),
        Err(error) => {
            tracing::error!(%error, "Failed to create tenant");
            HttpResponse::Conflict().json(serde_json::json!({
                "success": false,
                "error": "Tenant creation failed"
            }))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/tenants/{tenant_id}/users",
    request_body = CreateTenantUserRequest,
    params(("tenant_id" = i32, Path, description = "Tenant id")),
    responses(
        (status = 200, description = "User created", body = AdminUserResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    tag = "Admin"
)]
#[post("/admin/tenants/{tenant_id}/users")]
pub async fn create_tenant_user(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
    body: web::Json<CreateTenantUserRequest>,
) -> HttpResponse {
    if !current.0.is_superadmin {
        return forbidden();
    }

    let tenant_id = path.into_inner();
    let tenant_exists = match tenant::Entity::find_by_id(tenant_id).one(db.get_ref()).await {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(error) => {
            tracing::error!(%error, "Failed to load tenant");
            return server_error("Tenant lookup failed");
        }
    };

    if !tenant_exists {
        return HttpResponse::NotFound().json(serde_json::json!({
            "success": false,
            "error": "Tenant not found"
        }));
    }

    let password_hash = match hash_password(&body.password) {
        Ok(hash) => hash,
        Err(error) => {
            tracing::error!(%error, "Failed to hash password");
            return server_error("User creation failed");
        }
    };

    let new_user = user::ActiveModel {
        email: Set(body.email.clone()),
        name: Set(body.name.clone()),
        password_hash: Set(password_hash),
        tenant_id: Set(Some(tenant_id)),
        is_superadmin: Set(false),
        is_active: Set(true),
        ..Default::default()
    };

    let created_user = match new_user.insert(db.get_ref()).await {
        Ok(user) => user,
        Err(error) => {
            tracing::error!(%error, "Failed to create tenant user");
            return HttpResponse::Conflict().json(serde_json::json!({
                "success": false,
                "error": "User creation failed"
            }));
        }
    };

    if let Err(error) = attach_default_role(db.get_ref(), tenant_id, created_user.id).await {
        tracing::warn!(%error, "Failed to attach default tenant role");
    }

    HttpResponse::Ok().json(AdminUserResponse {
        id: created_user.id,
        email: created_user.email,
        name: created_user.name,
        tenant_id: created_user.tenant_id,
        is_superadmin: created_user.is_superadmin,
        is_active: created_user.is_active,
    })
}

async fn attach_default_role(
    db: &DatabaseConnection,
    tenant_id: i32,
    user_id: i32,
) -> Result<(), sea_orm::DbErr> {
    let Some(default_role) = role::Entity::find()
        .filter(role::Column::TenantId.eq(tenant_id))
        .filter(role::Column::Name.is_in(["agent", "admin", "user"]))
        .order_by_asc(role::Column::Id)
        .one(db)
        .await?
    else {
        return Ok(());
    };

    let existing = user_role::Entity::find()
        .filter(user_role::Column::UserId.eq(user_id))
        .filter(user_role::Column::RoleId.eq(default_role.id))
        .one(db)
        .await?;

    if existing.is_none() {
        let link = user_role::ActiveModel {
            user_id: Set(user_id),
            role_id: Set(default_role.id),
        };
        link.insert(db).await?;
    }

    Ok(())
}

fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().json(serde_json::json!({
        "success": false,
        "error": "Forbidden"
    }))
}

fn server_error(error: &str) -> HttpResponse {
    HttpResponse::InternalServerError().json(serde_json::json!({
        "success": false,
        "error": error
    }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(create_tenant).service(create_tenant_user);
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, web, App};
    use sea_orm::Database;

    use crate::{
        auth::jwt::{build_claims, encode_jwt},
        config::AppConfig,
    };

    async fn post_tenant(token: Option<String>, name: &str, slug: &str) -> (StatusCode, serde_json::Value) {
        let _ = dotenvy::dotenv();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL for admin tests");
        let db = Database::connect(&database_url).await.expect("db connect");
        let config = AppConfig {
            database_url,
            jwt_secret: "test-admin-secret".to_string(),
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            wa_phone_number_id: String::new(),
            wa_access_token: String::new(),
            wa_verify_token: String::new(),
            wa_api_version: "v25.0".to_string(),
        };

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(db))
                .app_data(web::Data::new(config))
                .configure(super::configure),
        )
        .await;

        let mut req = test::TestRequest::post()
            .uri("/admin/tenants")
            .set_json(&serde_json::json!({"name": name, "slug": slug}));
        if let Some(token) = token {
            req = req.insert_header(("Authorization", format!("Bearer {token}")));
        }

        let resp = test::call_service(&app, req.to_request()).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        let json = serde_json::from_slice(&body).unwrap_or_else(|_| serde_json::json!({}));
        (status, json)
    }

    fn admin_token() -> String {
        token_for_user(1)
    }

    fn token_for_user(user_id: i32) -> String {
        let claims = build_claims(user_id, None, true, 3600);
        encode_jwt(&claims, "test-admin-secret").expect("token")
    }

    async fn post_tenant_user(
        token: Option<String>,
        tenant_id: i32,
        email: &str,
    ) -> (StatusCode, serde_json::Value) {
        let _ = dotenvy::dotenv();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL for admin tests");
        let db = Database::connect(&database_url).await.expect("db connect");
        let config = AppConfig {
            database_url,
            jwt_secret: "test-admin-secret".to_string(),
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            wa_phone_number_id: String::new(),
            wa_access_token: String::new(),
            wa_verify_token: String::new(),
            wa_api_version: "v25.0".to_string(),
        };

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(db))
                .app_data(web::Data::new(config))
                .configure(super::configure),
        )
        .await;

        let mut req = test::TestRequest::post()
            .uri(&format!("/admin/tenants/{tenant_id}/users"))
            .set_json(&serde_json::json!({
                "email": email,
                "password": "agent123456",
                "name": "Agent Test"
            }));
        if let Some(token) = token {
            req = req.insert_header(("Authorization", format!("Bearer {token}")));
        }

        let resp = test::call_service(&app, req.to_request()).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        let json = serde_json::from_slice(&body).unwrap_or_else(|_| serde_json::json!({}));
        (status, json)
    }

    #[actix_rt::test]
    async fn superadmin_can_create_tenant() {
        let slug = format!("test-tenant-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap());
        let (status, body) = post_tenant(Some(admin_token()), "Test Tenant", &slug).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["slug"], slug);
    }

    #[actix_rt::test]
    async fn missing_token_cannot_create_tenant() {
        let (status, _) = post_tenant(None, "No Auth", "no-auth").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[actix_rt::test]
    async fn superadmin_can_create_tenant_user() {
        let suffix = chrono::Utc::now().timestamp_nanos_opt().unwrap();
        let slug = format!("test-user-tenant-{suffix}");
        let (tenant_status, tenant_body) = post_tenant(Some(admin_token()), "User Tenant", &slug).await;
        assert_eq!(tenant_status, StatusCode::OK);
        let tenant_id = tenant_body["id"].as_i64().expect("tenant id") as i32;

        let email = format!("agent-{suffix}@example.com");
        let (user_status, user_body) = post_tenant_user(Some(admin_token()), tenant_id, &email).await;
        assert_eq!(user_status, StatusCode::OK);
        assert_eq!(user_body["email"], email);
        assert_eq!(user_body["tenant_id"], tenant_id);
        assert_eq!(user_body["is_superadmin"], false);
        assert_eq!(user_body["is_active"], true);
    }

    #[actix_rt::test]
    async fn tenant_user_cannot_create_tenant() {
        let suffix = chrono::Utc::now().timestamp_nanos_opt().unwrap();
        let slug = format!("test-forbidden-tenant-{suffix}");
        let (tenant_status, tenant_body) = post_tenant(Some(admin_token()), "Forbidden Tenant", &slug).await;
        assert_eq!(tenant_status, StatusCode::OK);
        let tenant_id = tenant_body["id"].as_i64().expect("tenant id") as i32;

        let email = format!("tenant-user-{suffix}@example.com");
        let (user_status, user_body) = post_tenant_user(Some(admin_token()), tenant_id, &email).await;
        assert_eq!(user_status, StatusCode::OK);
        let user_id = user_body["id"].as_i64().expect("user id") as i32;

        let denied_slug = format!("denied-{suffix}");
        let (denied_status, _) = post_tenant(Some(token_for_user(user_id)), "Denied", &denied_slug).await;
        assert_eq!(denied_status, StatusCode::FORBIDDEN);
    }
}

