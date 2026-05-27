use actix_web::{get, patch, post, web, HttpResponse};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::middleware::CurrentUser;
use crate::application::auth::require_permission;
use crate::application::tenants::{
    create_storage_config as create_storage_config_use_case,
    create_whatsapp_account as create_whatsapp_account_use_case,
    get_storage_config as get_storage_config_use_case,
    list_whatsapp_accounts as list_whatsapp_accounts_use_case,
    update_storage_config as update_storage_config_use_case,
    update_whatsapp_account as update_whatsapp_account_use_case, CreateStorageConfigInput,
    CreateWhatsAppAccountInput, PatchStorageConfigInput, PatchWhatsAppAccountInput,
};
use crate::domain::auth::permissions;
use crate::domain::tenants::{StorageSettings, WhatsAppSettings};
use crate::infrastructure::config::AppConfig;

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "id": 1,
    "tenant_id": 1,
    "phone_number_id": "123456789",
    "business_account_id": "987654321",
    "display_phone_number": "+6281234567890",
    "access_token_masked": "EAAG...xYzW",
    "verify_token_masked": "my-s...oken",
    "api_version": "v25.0",
    "is_active": true,
    "webhook_url": "https://crm.example.com/webhook/whatsapp/acme"
}))]
pub struct WhatsAppAccountResponse {
    pub id: i32,
    pub tenant_id: i32,
    pub phone_number_id: String,
    pub business_account_id: String,
    pub display_phone_number: Option<String>,
    pub access_token_masked: String,
    pub verify_token_masked: String,
    pub api_version: String,
    pub is_active: bool,
    pub webhook_url: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({
    "phone_number_id": "123456789",
    "business_account_id": "987654321",
    "display_phone_number": "+6281234567890",
    "access_token": "EAAGxxxx...",
    "verify_token": "my-secret-verify-token",
    "api_version": "v25.0",
    "is_active": true
}))]
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
#[schema(example = json!({"display_phone_number": "+6289876543210", "is_active": true}))]
pub struct PatchWhatsAppAccountRequest {
    pub phone_number_id: Option<String>,
    pub business_account_id: Option<String>,
    pub display_phone_number: Option<String>,
    pub access_token: Option<String>,
    pub verify_token: Option<String>,
    pub api_version: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "id": 1, "tenant_id": 1,
    "endpoint": "https://abc123.r2.cloudflarestorage.com",
    "region": "auto",
    "access_key_id": "AKIAIOSFODNN7EXAMPLE",
    "secret_access_key_masked": "wJal...9kLm",
    "bucket": "acme-crm-media",
    "public_base_url": "https://cdn.acme.com",
    "is_active": true
}))]
pub struct StorageConfigResponse {
    pub id: i32,
    pub tenant_id: i32,
    pub endpoint: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key_masked: String,
    pub bucket: String,
    pub public_base_url: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({
    "endpoint": "https://abc123.r2.cloudflarestorage.com",
    "region": "auto",
    "access_key_id": "AKIAIOSFODNN7EXAMPLE",
    "secret_access_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
    "bucket": "acme-crm-media",
    "public_base_url": "https://cdn.acme.com"
}))]
pub struct CreateStorageConfigRequest {
    pub endpoint: String,
    pub region: Option<String>,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
    pub public_base_url: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"bucket": "new-bucket", "is_active": true}))]
pub struct PatchStorageConfigRequest {
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub bucket: Option<String>,
    pub public_base_url: Option<String>,
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

    match list_whatsapp_accounts_use_case(db.get_ref(), tenant_id).await {
        Ok(result) => HttpResponse::Ok().json(
            result
                .accounts
                .into_iter()
                .map(|account| {
                    build_account_response(&account, &result.tenant_slug, &config.app_base_url)
                })
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

    match create_whatsapp_account_use_case(
        db.get_ref(),
        CreateWhatsAppAccountInput {
            tenant_id,
            phone_number_id: body.phone_number_id.clone(),
            business_account_id: body.business_account_id.clone(),
            display_phone_number: body.display_phone_number.clone(),
            access_token: body.access_token.clone(),
            verify_token: body.verify_token.clone(),
            api_version: body.api_version.clone(),
            is_active: body.is_active.unwrap_or(true),
        },
    )
    .await
    {
        Ok(result) => HttpResponse::Ok().json(build_account_response(
            &result.account,
            &result.tenant_slug,
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

    match update_whatsapp_account_use_case(
        db.get_ref(),
        PatchWhatsAppAccountInput {
            id: path.into_inner(),
            tenant_id,
            phone_number_id: body.phone_number_id.clone(),
            business_account_id: body.business_account_id.clone(),
            display_phone_number: body.display_phone_number.clone(),
            access_token: body.access_token.clone(),
            verify_token: body.verify_token.clone(),
            api_version: body.api_version.clone(),
            is_active: body.is_active,
        },
    )
    .await
    {
        Ok(Some(result)) => HttpResponse::Ok().json(build_account_response(
            &result.account,
            &result.tenant_slug,
            &config.app_base_url,
        )),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "success": false,
            "error": "WhatsApp settings not found"
        })),
        Err(error) => {
            tracing::error!(%error, "Failed to update WhatsApp account");
            server_error("Failed to update WhatsApp settings")
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/settings/storage",
    responses((status = 200, description = "Storage config", body = StorageConfigResponse),
              (status = 404, description = "No storage config")),
    tag = "Settings"
)]
#[get("/settings/storage")]
pub async fn get_storage_config(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
) -> HttpResponse {
    let ctx = &current.0;
    if let Err(response) = require_permission(ctx, permissions::SETTINGS_STORAGE_MANAGE) {
        return response;
    }
    let Some(tenant_id) = ctx.tenant_id else {
        return forbidden("Tenant context required");
    };

    match get_storage_config_use_case(db.get_ref(), tenant_id).await {
        Ok(Some(config)) => HttpResponse::Ok().json(build_storage_config_response(&config)),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "success": false,
            "error": "Storage configuration not found"
        })),
        Err(error) => {
            tracing::error!(%error, "Failed to load storage config");
            server_error("Failed to load storage configuration")
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/settings/storage",
    request_body = CreateStorageConfigRequest,
    responses((status = 200, description = "Storage config created", body = StorageConfigResponse)),
    tag = "Settings"
)]
#[post("/settings/storage")]
pub async fn create_storage_config(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    body: web::Json<CreateStorageConfigRequest>,
) -> HttpResponse {
    let ctx = &current.0;
    if let Err(response) = require_permission(ctx, permissions::SETTINGS_STORAGE_MANAGE) {
        return response;
    }
    let Some(tenant_id) = ctx.tenant_id else {
        return forbidden("Tenant context required");
    };

    match create_storage_config_use_case(
        db.get_ref(),
        CreateStorageConfigInput {
            tenant_id,
            endpoint: body.endpoint.clone(),
            region: body.region.clone(),
            access_key_id: body.access_key_id.clone(),
            secret_access_key: body.secret_access_key.clone(),
            bucket: body.bucket.clone(),
            public_base_url: body.public_base_url.clone(),
            is_active: body.is_active.unwrap_or(true),
        },
    )
    .await
    {
        Ok(config) => HttpResponse::Ok().json(build_storage_config_response(&config)),
        Err(error) => {
            tracing::error!(%error, "Failed to create storage config");
            HttpResponse::Conflict().json(serde_json::json!({
                "success": false,
                "error": "Failed to create storage configuration (may already exist)"
            }))
        }
    }
}

#[utoipa::path(
    patch,
    path = "/api/settings/storage",
    request_body = PatchStorageConfigRequest,
    responses((status = 200, description = "Storage config updated", body = StorageConfigResponse)),
    tag = "Settings"
)]
#[patch("/settings/storage")]
pub async fn update_storage_config(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    body: web::Json<PatchStorageConfigRequest>,
) -> HttpResponse {
    let ctx = &current.0;
    if let Err(response) = require_permission(ctx, permissions::SETTINGS_STORAGE_MANAGE) {
        return response;
    }
    let Some(tenant_id) = ctx.tenant_id else {
        return forbidden("Tenant context required");
    };

    match update_storage_config_use_case(
        db.get_ref(),
        PatchStorageConfigInput {
            tenant_id,
            endpoint: body.endpoint.clone(),
            region: body.region.clone(),
            access_key_id: body.access_key_id.clone(),
            secret_access_key: body.secret_access_key.clone(),
            bucket: body.bucket.clone(),
            public_base_url: body.public_base_url.clone(),
            is_active: body.is_active,
        },
    )
    .await
    {
        Ok(Some(config)) => HttpResponse::Ok().json(build_storage_config_response(&config)),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "success": false,
            "error": "Storage configuration not found"
        })),
        Err(error) => {
            tracing::error!(%error, "Failed to update storage config");
            server_error("Failed to update storage configuration")
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
    account: &WhatsAppSettings,
    tenant_slug: &str,
    app_base_url: &str,
) -> WhatsAppAccountResponse {
    WhatsAppAccountResponse {
        id: account.id(),
        tenant_id: account.tenant_id(),
        phone_number_id: account.phone_number_id().to_string(),
        business_account_id: account.business_account_id().to_string(),
        display_phone_number: account.display_phone_number().map(ToString::to_string),
        access_token_masked: mask_token(account.access_token()),
        verify_token_masked: mask_token(account.verify_token()),
        api_version: account.api_version().to_string(),
        is_active: account.is_active(),
        webhook_url: format!(
            "{}/webhook/whatsapp/{}",
            app_base_url.trim_end_matches('/'),
            tenant_slug
        ),
    }
}

fn build_storage_config_response(row: &StorageSettings) -> StorageConfigResponse {
    StorageConfigResponse {
        id: row.id(),
        tenant_id: row.tenant_id(),
        endpoint: row.endpoint().to_string(),
        region: row.region().to_string(),
        access_key_id: row.access_key_id().to_string(),
        secret_access_key_masked: mask_token(row.secret_access_key()),
        bucket: row.bucket().to_string(),
        public_base_url: row.public_base_url().map(ToString::to_string),
        is_active: row.is_active(),
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
        .service(update_whatsapp_account)
        .service(get_storage_config)
        .service(create_storage_config)
        .service(update_storage_config);
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test as awtest, web, App};
    use sea_orm::{
        ActiveModelTrait, ActiveValue::Set, ColumnTrait, Database, EntityTrait, QueryFilter,
    };

    use super::mask_token;
    use crate::domain::auth::permissions;
    use crate::infrastructure::config::AppConfig;
    use crate::infrastructure::persistence::models::{
        permission, role, role_permission, tenant, user, user_role,
    };
    use crate::infrastructure::security::hash_password;
    use crate::infrastructure::security::{build_claims, encode_jwt};

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
            .set_json(serde_json::json!({
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
            .set_json(serde_json::json!({
                "access_token": "updated-token-9876",
                "is_active": false
            }))
            .to_request();
        let patch_resp = awtest::call_service(&app, patch_req).await;
        assert_eq!(patch_resp.status(), StatusCode::OK);
        let patch_body: serde_json::Value =
            serde_json::from_slice(&awtest::read_body(patch_resp).await).unwrap();
        assert_eq!(patch_body["id"], account_id);
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
