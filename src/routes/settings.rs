use actix_web::{get, patch, post, web, HttpResponse};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::CurrentUser;
use crate::config::AppConfig;
use crate::models::{tenant, tenant_whatsapp_account};
use crate::rbac::{permissions, require_permission};

#[derive(Debug, Serialize, ToSchema)]
pub struct WhatsAppAccountResponse {
    pub id: i32,
    pub tenant_id: i32,
    pub phone_number_id: String,
    pub business_account_id: String,
    pub display_phone_number: Option<String>,
    pub access_token_masked: String,
    pub verify_token: String,
    pub api_version: String,
    pub is_active: bool,
    pub webhook_url: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertWhatsAppAccountRequest {
    pub phone_number_id: String,
    pub business_account_id: String,
    pub display_phone_number: Option<String>,
    pub access_token: String,
    pub verify_token: String,
    pub api_version: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchWhatsAppAccountRequest {
    pub phone_number_id: Option<String>,
    pub business_account_id: Option<String>,
    pub display_phone_number: Option<String>,
    pub access_token: Option<String>,
    pub verify_token: Option<String>,
    pub api_version: Option<String>,
    pub is_active: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/api/settings/whatsapp",
    responses((status = 200, description = "WhatsApp accounts", body = [WhatsAppAccountResponse])),
    tag = "Settings"
)]
#[get("/settings/whatsapp")]
pub async fn list_whatsapp_accounts(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    config: web::Data<AppConfig>,
) -> HttpResponse {
    let ctx = &current.0;
    if let Err(response) = require_permission(ctx, permissions::SETTINGS_WHATSAPP_MANAGE) {
        return response;
    }
    let Some(tenant_id) = ctx.tenant_id else {
        return forbidden("Tenant context required");
    };

    let tenant = match tenant::Entity::find_by_id(tenant_id)
        .one(db.get_ref())
        .await
    {
        Ok(Some(tenant)) => tenant,
        Ok(None) => return server_error("Tenant not found"),
        Err(error) => {
            tracing::error!(%error, "Failed to load tenant");
            return server_error("Failed to load tenant");
        }
    };

    match tenant_whatsapp_account::Entity::find()
        .filter(tenant_whatsapp_account::Column::TenantId.eq(tenant_id))
        .all(db.get_ref())
        .await
    {
        Ok(accounts) => HttpResponse::Ok().json(
            accounts
                .into_iter()
                .map(|account| build_account_response(&account, &tenant.slug, &config.app_base_url))
                .collect::<Vec<_>>(),
        ),
        Err(error) => {
            tracing::error!(%error, "Failed to list WhatsApp accounts");
            server_error("Failed to list WhatsApp settings")
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/settings/whatsapp",
    request_body = UpsertWhatsAppAccountRequest,
    responses((status = 200, description = "WhatsApp account created", body = WhatsAppAccountResponse)),
    tag = "Settings"
)]
#[post("/settings/whatsapp")]
pub async fn create_whatsapp_account(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    config: web::Data<AppConfig>,
    body: web::Json<UpsertWhatsAppAccountRequest>,
) -> HttpResponse {
    let ctx = &current.0;
    if let Err(response) = require_permission(ctx, permissions::SETTINGS_WHATSAPP_MANAGE) {
        return response;
    }
    let Some(tenant_id) = ctx.tenant_id else {
        return forbidden("Tenant context required");
    };

    let tenant = match tenant::Entity::find_by_id(tenant_id)
        .one(db.get_ref())
        .await
    {
        Ok(Some(tenant)) => tenant,
        Ok(None) => return server_error("Tenant not found"),
        Err(error) => {
            tracing::error!(%error, "Failed to load tenant");
            return server_error("Failed to load tenant");
        }
    };

    let model = tenant_whatsapp_account::ActiveModel {
        tenant_id: Set(tenant_id),
        phone_number_id: Set(body.phone_number_id.clone()),
        business_account_id: Set(body.business_account_id.clone()),
        display_phone_number: Set(body.display_phone_number.clone()),
        access_token: Set(body.access_token.clone()),
        verify_token: Set(body.verify_token.clone()),
        api_version: Set(body
            .api_version
            .clone()
            .unwrap_or_else(|| "v25.0".to_string())),
        is_active: Set(body.is_active.unwrap_or(true)),
        ..Default::default()
    };

    match model.insert(db.get_ref()).await {
        Ok(account) => HttpResponse::Ok().json(build_account_response(
            &account,
            &tenant.slug,
            &config.app_base_url,
        )),
        Err(error) => {
            tracing::error!(%error, "Failed to create WhatsApp account");
            HttpResponse::Conflict().json(serde_json::json!({
                "success": false,
                "error": "Failed to create WhatsApp settings"
            }))
        }
    }
}

#[utoipa::path(
    patch,
    path = "/api/settings/whatsapp/{id}",
    request_body = PatchWhatsAppAccountRequest,
    params(("id" = i32, Path, description = "WhatsApp account id")),
    responses((status = 200, description = "WhatsApp account updated", body = WhatsAppAccountResponse)),
    tag = "Settings"
)]
#[patch("/settings/whatsapp/{id}")]
pub async fn update_whatsapp_account(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    config: web::Data<AppConfig>,
    path: web::Path<i32>,
    body: web::Json<PatchWhatsAppAccountRequest>,
) -> HttpResponse {
    let ctx = &current.0;
    if let Err(response) = require_permission(ctx, permissions::SETTINGS_WHATSAPP_MANAGE) {
        return response;
    }
    let Some(tenant_id) = ctx.tenant_id else {
        return forbidden("Tenant context required");
    };

    let tenant = match tenant::Entity::find_by_id(tenant_id)
        .one(db.get_ref())
        .await
    {
        Ok(Some(tenant)) => tenant,
        Ok(None) => return server_error("Tenant not found"),
        Err(error) => {
            tracing::error!(%error, "Failed to load tenant");
            return server_error("Failed to load tenant");
        }
    };

    let id = path.into_inner();
    let account = match tenant_whatsapp_account::Entity::find()
        .filter(tenant_whatsapp_account::Column::Id.eq(id))
        .filter(tenant_whatsapp_account::Column::TenantId.eq(tenant_id))
        .one(db.get_ref())
        .await
    {
        Ok(Some(account)) => account,
        Ok(None) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "error": "WhatsApp settings not found"
            }));
        }
        Err(error) => {
            tracing::error!(%error, "Failed to load WhatsApp account");
            return server_error("Failed to load WhatsApp settings");
        }
    };

    let mut active: tenant_whatsapp_account::ActiveModel = account.into();
    if let Some(value) = &body.phone_number_id {
        active.phone_number_id = Set(value.clone());
    }
    if let Some(value) = &body.business_account_id {
        active.business_account_id = Set(value.clone());
    }
    if let Some(value) = &body.display_phone_number {
        active.display_phone_number = Set(Some(value.clone()));
    }
    if let Some(value) = &body.access_token {
        active.access_token = Set(value.clone());
    }
    if let Some(value) = &body.verify_token {
        active.verify_token = Set(value.clone());
    }
    if let Some(value) = &body.api_version {
        active.api_version = Set(value.clone());
    }
    if let Some(value) = body.is_active {
        active.is_active = Set(value);
    }

    match active.update(db.get_ref()).await {
        Ok(account) => HttpResponse::Ok().json(build_account_response(
            &account,
            &tenant.slug,
            &config.app_base_url,
        )),
        Err(error) => {
            tracing::error!(%error, "Failed to update WhatsApp account");
            server_error("Failed to update WhatsApp settings")
        }
    }
}

pub fn mask_token(token: &str) -> String {
    let chars = token.chars().collect::<Vec<_>>();
    if chars.len() <= 8 {
        "*".repeat(chars.len())
    } else {
        let prefix = chars.iter().take(4).collect::<String>();
        let suffix = chars.iter().skip(chars.len() - 4).collect::<String>();
        format!("{prefix}...{suffix}")
    }
}

fn build_account_response(
    account: &tenant_whatsapp_account::Model,
    tenant_slug: &str,
    app_base_url: &str,
) -> WhatsAppAccountResponse {
    WhatsAppAccountResponse {
        id: account.id,
        tenant_id: account.tenant_id,
        phone_number_id: account.phone_number_id.clone(),
        business_account_id: account.business_account_id.clone(),
        display_phone_number: account.display_phone_number.clone(),
        access_token_masked: mask_token(&account.access_token),
        verify_token: account.verify_token.clone(),
        api_version: account.api_version.clone(),
        is_active: account.is_active,
        webhook_url: format!(
            "{}/webhook/whatsapp/{}",
            app_base_url.trim_end_matches('/'),
            tenant_slug
        ),
    }
}

fn forbidden(message: &str) -> HttpResponse {
    HttpResponse::Forbidden().json(serde_json::json!({
        "success": false,
        "error": message
    }))
}

fn server_error(message: &str) -> HttpResponse {
    HttpResponse::InternalServerError().json(serde_json::json!({
        "success": false,
        "error": message
    }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_whatsapp_accounts)
        .service(create_whatsapp_account)
        .service(update_whatsapp_account);
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test as awtest, web, App};
    use sea_orm::{
        ActiveModelTrait, ActiveValue::Set, ColumnTrait, Database, EntityTrait, QueryFilter,
    };

    use super::mask_token;
    use crate::auth::jwt::{build_claims, encode_jwt};
    use crate::auth::password::hash_password;
    use crate::config::AppConfig;
    use crate::models::{permission, role, role_permission, tenant, user, user_role};
    use crate::rbac::permissions;

    #[test]
    fn masks_long_access_token() {
        assert_eq!(mask_token("abcdef123456"), "abcd...3456");
    }

    #[test]
    fn fully_masks_short_access_token() {
        assert_eq!(mask_token("secret"), "******");
    }

    #[test]
    fn masks_unicode_without_panicking() {
        assert_eq!(mask_token("åß∂ƒ©˙∆˚¬µ"), "åß∂ƒ...∆˚¬µ");
    }

    async fn setup_settings_user() -> (String, i32) {
        let _ = dotenvy::dotenv();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL for settings tests");
        let db = Database::connect(&database_url).await.expect("db connect");
        let suffix = chrono::Utc::now().timestamp_nanos_opt().unwrap();

        let tenant = tenant::ActiveModel {
            name: Set("Settings Test Tenant".to_string()),
            slug: Set(format!("settings-test-{suffix}")),
            is_active: Set(true),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("tenant");

        let user = user::ActiveModel {
            email: Set(format!("settings-user-{suffix}@example.com")),
            name: Set("Settings User".to_string()),
            password_hash: Set(hash_password("settings123456").expect("hash")),
            tenant_id: Set(Some(tenant.id)),
            is_superadmin: Set(false),
            is_active: Set(true),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("user");

        let permission = match permission::Entity::find()
            .filter(permission::Column::Code.eq(permissions::SETTINGS_WHATSAPP_MANAGE))
            .one(&db)
            .await
            .expect("permission lookup")
        {
            Some(permission) => permission,
            None => permission::ActiveModel {
                code: Set(permissions::SETTINGS_WHATSAPP_MANAGE.to_string()),
                description: Set(Some("Manage WhatsApp account settings".to_string())),
                ..Default::default()
            }
            .insert(&db)
            .await
            .expect("permission"),
        };

        let role = role::ActiveModel {
            tenant_id: Set(Some(tenant.id)),
            name: Set("settings-manager".to_string()),
            description: Set(Some("Settings manager".to_string())),
            is_system: Set(false),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("role");

        role_permission::ActiveModel {
            role_id: Set(role.id),
            permission_id: Set(permission.id),
        }
        .insert(&db)
        .await
        .expect("role permission");

        user_role::ActiveModel {
            user_id: Set(user.id),
            role_id: Set(role.id),
        }
        .insert(&db)
        .await
        .expect("user role");

        let token = encode_jwt(
            &build_claims(user.id, Some(tenant.id), false, 3600),
            "test-settings-secret",
        )
        .expect("token");
        (token, tenant.id)
    }

    #[actix_rt::test]
    async fn create_and_list_whatsapp_account_masks_access_token() {
        let (token, tenant_id) = setup_settings_user().await;
        let _ = dotenvy::dotenv();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL for settings tests");
        let db = Database::connect(&database_url).await.expect("db connect");
        let config = AppConfig {
            database_url,
            jwt_secret: "test-settings-secret".to_string(),
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
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(db))
                .app_data(web::Data::new(config))
                .configure(super::configure),
        )
        .await;
        let phone_number_id = format!(
            "phone-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        );

        let create_req = awtest::TestRequest::post()
            .uri("/settings/whatsapp")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .set_json(&serde_json::json!({
                "phone_number_id": phone_number_id,
                "business_account_id": "biz-123",
                "display_phone_number": "62800000000",
                "access_token": "abcdef123456",
                "verify_token": "verify-123",
                "api_version": "v25.0"
            }))
            .to_request();

        let create_resp = awtest::call_service(&app, create_req).await;
        assert_eq!(create_resp.status(), StatusCode::OK);
        let create_body: serde_json::Value =
            serde_json::from_slice(&awtest::read_body(create_resp).await).unwrap();
        assert_eq!(create_body["tenant_id"], tenant_id);
        assert_eq!(create_body["access_token_masked"], "abcd...3456");
        assert!(create_body.get("access_token").is_none());
        let account_id = create_body["id"].as_i64().expect("account id");

        let patch_req = awtest::TestRequest::patch()
            .uri(&format!("/settings/whatsapp/{account_id}"))
            .insert_header(("Authorization", format!("Bearer {token}")))
            .set_json(&serde_json::json!({
                "access_token": "updated-token-9876",
                "is_active": false
            }))
            .to_request();
        let patch_resp = awtest::call_service(&app, patch_req).await;
        assert_eq!(patch_resp.status(), StatusCode::OK);
        let patch_body: serde_json::Value =
            serde_json::from_slice(&awtest::read_body(patch_resp).await).unwrap();
        assert_eq!(patch_body["access_token_masked"], "upda...9876");
        assert_eq!(patch_body["is_active"], false);
        assert!(patch_body.get("access_token").is_none());

        let list_req = awtest::TestRequest::get()
            .uri("/settings/whatsapp")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let list_resp = awtest::call_service(&app, list_req).await;
        assert_eq!(list_resp.status(), StatusCode::OK);
        let list_body: serde_json::Value =
            serde_json::from_slice(&awtest::read_body(list_resp).await).unwrap();
        assert!(list_body
            .as_array()
            .unwrap()
            .iter()
            .all(|account| account["tenant_id"] == tenant_id));
        assert!(list_body
            .as_array()
            .unwrap()
            .iter()
            .all(|account| account.get("access_token").is_none()));
    }
}
