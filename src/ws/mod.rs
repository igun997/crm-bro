pub mod hub;
pub mod session;

use actix_web::web;
use actix_web::{get, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::auth::jwt::decode_jwt;
use crate::config::AppConfig;
use crate::models::conversation;
use hub::ChatHub;

/// WebSocket endpoint for global updates (all new messages)
#[get("/ws/updates")]
pub async fn ws_updates(
    req: HttpRequest,
    stream: web::Payload,
    hub: web::Data<actix::Addr<ChatHub>>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant_id_from_request(&req, &config)?;
    let session = session::ChatSession::new(hub.get_ref().clone(), tenant_id, None);
    ws::start(session, &req, stream)
}

/// WebSocket endpoint for specific conversation
#[get("/ws/chat/{conversation_id}")]
pub async fn ws_chat(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<i32>,
    hub: web::Data<actix::Addr<ChatHub>>,
    config: web::Data<AppConfig>,
    db: web::Data<DatabaseConnection>,
) -> Result<HttpResponse, actix_web::Error> {
    let conversation_id = path.into_inner();
    let tenant_id = tenant_id_from_request(&req, &config)?;
    ensure_conversation_tenant(db.get_ref(), conversation_id, tenant_id).await?;
    let session =
        session::ChatSession::new(hub.get_ref().clone(), tenant_id, Some(conversation_id));
    ws::start(session, &req, stream)
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_updates).service(ws_chat);
}

fn percent_decode(input: &str) -> Result<String, ()> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).map_err(|_| ())?;
                let value = u8::from_str_radix(hex, 16).map_err(|_| ())?;
                output.push(value);
                i += 3;
            }
            b'+' => {
                output.push(b' ');
                i += 1;
            }
            byte => {
                output.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(output).map_err(|_| ())
}

fn tenant_id_from_request(req: &HttpRequest, config: &AppConfig) -> Result<i32, actix_web::Error> {
    let token = req
        .query_string()
        .split('&')
        .find_map(|pair| pair.strip_prefix("token="))
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("Missing token"))?;
    let token =
        percent_decode(token).map_err(|_| actix_web::error::ErrorUnauthorized("Invalid token"))?;
    let claims = decode_jwt(&token, &config.jwt_secret)
        .map_err(|_| actix_web::error::ErrorUnauthorized("Invalid token"))?
        .claims;
    claims
        .tenant_id
        .ok_or_else(|| actix_web::error::ErrorForbidden("Tenant required"))
}

async fn ensure_conversation_tenant(
    db: &DatabaseConnection,
    conversation_id: i32,
    tenant_id: i32,
) -> Result<(), actix_web::Error> {
    let found = conversation::Entity::find_by_id(conversation_id)
        .filter(conversation::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError("Failed to load conversation"))?;

    if found.is_some() {
        Ok(())
    } else {
        Err(actix_web::error::ErrorForbidden("Conversation forbidden"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::{build_claims, encode_jwt};

    fn test_config() -> AppConfig {
        AppConfig {
            database_url: "mysql://test:test@localhost/test".into(),
            server_host: "127.0.0.1".into(),
            server_port: 8080,
            jwt_secret: "test-secret".into(),
            app_base_url: "http://localhost:8080".into(),
            storage_backend: "local".into(),
            storage_local_dir: "media".into(),
            r2_endpoint: None,
            r2_access_key_id: None,
            r2_secret_access_key: None,
            r2_bucket: None,
            r2_public_base_url: None,
        }
    }

    #[test]
    fn tenant_id_from_request_reads_jwt_query_token() {
        let config = test_config();
        let claims = build_claims(1, Some(42), false, 3600);
        let token = encode_jwt(&claims, &config.jwt_secret).unwrap();
        let req = actix_web::test::TestRequest::with_uri(&format!("/ws/updates?token={token}"))
            .to_http_request();

        assert_eq!(tenant_id_from_request(&req, &config).unwrap(), 42);
    }

    #[test]
    fn tenant_id_from_request_rejects_missing_token() {
        let config = test_config();
        let req = actix_web::test::TestRequest::with_uri("/ws/updates").to_http_request();

        assert!(tenant_id_from_request(&req, &config).is_err());
    }
}
