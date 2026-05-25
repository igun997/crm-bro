use actix_web::{dev::Payload, error::ErrorUnauthorized, web, Error, FromRequest, HttpRequest};
use futures_util::future::LocalBoxFuture;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::HashSet;

use crate::auth::context::AuthContext;
use crate::auth::jwt::decode_jwt;
use crate::infrastructure::config::AppConfig;
use crate::middleware::extract_bearer;
use crate::models::{permission, role_permission, user, user_role};

#[derive(Debug, Clone)]
pub struct CurrentUser(pub AuthContext);

impl FromRequest for CurrentUser {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let auth_header = req
            .headers()
            .get(actix_web::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let config = req.app_data::<web::Data<AppConfig>>().cloned();
        let db = req.app_data::<web::Data<DatabaseConnection>>().cloned();

        Box::pin(async move {
            let token = auth_header
                .as_deref()
                .and_then(extract_bearer)
                .ok_or_else(|| ErrorUnauthorized("Missing bearer token"))?;
            let config = config.ok_or_else(|| ErrorUnauthorized("Server config missing"))?;
            let db = db.ok_or_else(|| ErrorUnauthorized("Database missing"))?;

            let claims = decode_jwt(token, &config.jwt_secret)
                .map_err(|_| ErrorUnauthorized("Invalid token"))?
                .claims;

            let user = user::Entity::find_by_id(claims.sub)
                .one(db.get_ref())
                .await
                .map_err(|_| ErrorUnauthorized("Invalid token"))?
                .ok_or_else(|| ErrorUnauthorized("Invalid token"))?;

            if !user.is_active {
                return Err(ErrorUnauthorized("Inactive user"));
            }

            let permissions = load_permissions(db.get_ref(), user.id)
                .await
                .map_err(|_| ErrorUnauthorized("Invalid token"))?;

            Ok(CurrentUser(AuthContext {
                user_id: user.id,
                tenant_id: user.tenant_id,
                is_superadmin: user.is_superadmin,
                permissions,
            }))
        })
    }
}

async fn load_permissions(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<HashSet<String>, sea_orm::DbErr> {
    let role_ids = user_role::Entity::find()
        .filter(user_role::Column::UserId.eq(user_id))
        .all(db)
        .await?
        .into_iter()
        .map(|user_role| user_role.role_id)
        .collect::<Vec<_>>();

    if role_ids.is_empty() {
        return Ok(HashSet::new());
    }

    let permission_ids = role_permission::Entity::find()
        .filter(role_permission::Column::RoleId.is_in(role_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|role_permission| role_permission.permission_id)
        .collect::<Vec<_>>();

    if permission_ids.is_empty() {
        return Ok(HashSet::new());
    }

    let permissions = permission::Entity::find()
        .filter(permission::Column::Id.is_in(permission_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|permission| permission.code)
        .collect::<HashSet<_>>();

    Ok(permissions)
}

impl From<AuthContext> for CurrentUser {
    fn from(ctx: AuthContext) -> Self {
        Self(ctx)
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, web, App, HttpResponse};
    use sea_orm::Database;

    use super::CurrentUser;
    use crate::{
        auth::jwt::{build_claims, encode_jwt},
        infrastructure::config::AppConfig,
    };

    async fn whoami(current: CurrentUser) -> HttpResponse {
        HttpResponse::Ok().json(serde_json::json!({
            "user_id": current.0.user_id,
            "tenant_id": current.0.tenant_id,
            "is_superadmin": current.0.is_superadmin,
            "has_system_manage": current.0.permissions.contains("system.manage")
                || current.0.permissions.contains("admin.tenants.manage"),
        }))
    }

    async fn call_whoami(token: Option<String>) -> (StatusCode, serde_json::Value) {
        let _ = dotenvy::dotenv();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL for extractor test");
        let db = Database::connect(&database_url).await.expect("db connect");
        let config = AppConfig {
            database_url,
            jwt_secret: "test-extractor-secret".to_string(),
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
                .route("/whoami", web::get().to(whoami)),
        )
        .await;

        let mut req = test::TestRequest::get().uri("/whoami");
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
    async fn accepts_valid_bearer_token_for_seeded_admin() {
        let claims = build_claims(1, None, true, 3600);
        let token = encode_jwt(&claims, "test-extractor-secret").expect("token");
        let (status, body) = call_whoami(Some(token)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["user_id"], 1);
        assert_eq!(body["tenant_id"], serde_json::Value::Null);
        assert_eq!(body["is_superadmin"], true);
        assert_eq!(body["has_system_manage"], true);
    }

    #[actix_rt::test]
    async fn loads_authority_from_database_not_jwt_flags() {
        let claims = build_claims(1, Some(999), false, 3600);
        let token = encode_jwt(&claims, "test-extractor-secret").expect("token");
        let (status, body) = call_whoami(Some(token)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["tenant_id"], serde_json::Value::Null);
        assert_eq!(body["is_superadmin"], true);
    }

    #[actix_rt::test]
    async fn rejects_missing_bearer_token() {
        let (status, _) = call_whoami(None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
