use actix_web::{get, post, web, HttpResponse};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::application::messaging::{
    handle_inbound_message, handle_status_update, resolve_whatsapp_account,
};
use crate::infrastructure::persistence::models::tenant_whatsapp_account;
use crate::infrastructure::storage::StorageService;
use crate::infrastructure::websocket::ChatHub;

use crate::infrastructure::whatsapp::types::*;

#[derive(serde::Deserialize)]
pub struct WebhookPath {
    pub tenant_slug: String,
}

#[derive(serde::Deserialize)]
pub struct VerifyQuery {
    #[serde(rename = "hub.mode")]
    pub mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    pub verify_token: Option<String>,
    #[serde(rename = "hub.challenge")]
    pub challenge: Option<String>,
}

/// Webhook verification (Meta sends GET to verify)
#[get("/{tenant_slug}")]
pub async fn verify(
    path: web::Path<WebhookPath>,
    query: web::Query<VerifyQuery>,
    db: web::Data<DatabaseConnection>,
) -> HttpResponse {
    let mode = query.mode.as_deref().unwrap_or("");
    let token = query.verify_token.as_deref().unwrap_or("");
    let challenge = query.challenge.as_deref().unwrap_or("");

    if mode != "subscribe" {
        return HttpResponse::Forbidden().finish();
    }

    // Look up tenant by slug
    let tenant = match crate::infrastructure::persistence::models::tenant::Entity::find()
        .filter(
            crate::infrastructure::persistence::models::tenant::Column::Slug.eq(&path.tenant_slug),
        )
        .filter(crate::infrastructure::persistence::models::tenant::Column::IsActive.eq(true))
        .one(db.get_ref())
        .await
    {
        Ok(Some(t)) => t,
        _ => {
            tracing::warn!(slug = %path.tenant_slug, "Webhook verify: tenant not found");
            return HttpResponse::Forbidden().finish();
        }
    };

    // Look up tenant WhatsApp account
    let account = match tenant_whatsapp_account::Entity::find()
        .filter(tenant_whatsapp_account::Column::TenantId.eq(tenant.id))
        .filter(tenant_whatsapp_account::Column::IsActive.eq(true))
        .one(db.get_ref())
        .await
    {
        Ok(Some(a)) => a,
        _ => {
            tracing::warn!(tenant_id = tenant.id, "Webhook verify: no WA account");
            return HttpResponse::Forbidden().finish();
        }
    };

    if token != account.verify_token {
        tracing::warn!(slug = %path.tenant_slug, "Webhook verify: token mismatch");
        return HttpResponse::Forbidden().finish();
    }

    tracing::info!(slug = %path.tenant_slug, "Webhook verified");
    HttpResponse::Ok().body(challenge.to_string())
}

/// Receive messages/status updates from Meta
#[post("/{tenant_slug}")]
pub async fn receive(
    path: web::Path<WebhookPath>,
    body: web::Json<WebhookPayload>,
    db: web::Data<DatabaseConnection>,
    storage: web::Data<StorageService>,
    hub: web::Data<actix::Addr<ChatHub>>,
) -> HttpResponse {
    // Look up tenant by slug — return 200 to Meta even if not found
    let tenant = match crate::infrastructure::persistence::models::tenant::Entity::find()
        .filter(
            crate::infrastructure::persistence::models::tenant::Column::Slug.eq(&path.tenant_slug),
        )
        .filter(crate::infrastructure::persistence::models::tenant::Column::IsActive.eq(true))
        .one(db.get_ref())
        .await
    {
        Ok(Some(t)) => t,
        _ => {
            tracing::warn!(slug = %path.tenant_slug, "Webhook receive: tenant not found, skipping");
            return HttpResponse::Ok().finish();
        }
    };

    for entry in &body.entry {
        for change in &entry.changes {
            if change.field != "messages" {
                continue;
            }

            let account = match resolve_whatsapp_account(db.get_ref(), change).await {
                Ok(Some(account)) => account,
                Ok(None) => {
                    tracing::warn!(
                        "Webhook change has no active tenant WhatsApp account; skipping"
                    );
                    continue;
                }
                Err(error) => {
                    tracing::error!(%error, "Failed to resolve tenant WhatsApp account");
                    continue;
                }
            };

            // Cross-check that the resolved account belongs to the correct tenant
            if account.tenant_id != tenant.id {
                tracing::warn!(
                    slug = %path.tenant_slug,
                    account_tenant = account.tenant_id,
                    "Webhook phone_number_id does not match tenant slug"
                );
                continue;
            }

            // Resolve tenant-specific storage with fallback to global
            let tenant_storage =
                match StorageService::resolve_for_tenant(db.get_ref(), tenant.id).await {
                    Ok(ts) => ts,
                    Err(error) => {
                        tracing::error!(%error, "Failed to resolve tenant storage");
                        None
                    }
                };
            let effective_storage = tenant_storage.as_ref().unwrap_or(storage.get_ref());

            if let Some(messages) = &change.value.messages {
                for msg in messages {
                    let contact_name = contact_name_for_message(change, msg);
                    let phone = contact_phone_for_message(change, msg);
                    if let Err(error) = handle_inbound_message(
                        db.get_ref(),
                        effective_storage,
                        &hub,
                        &account,
                        msg,
                        &phone,
                        &contact_name,
                    )
                    .await
                    {
                        tracing::error!("Failed to handle message {}: {}", msg.id, error);
                    }
                }
            }

            if let Some(statuses) = &change.value.statuses {
                for status in statuses {
                    if let Err(error) = handle_status_update(db.get_ref(), status).await {
                        tracing::error!("Failed to handle status {}: {}", status.id, error);
                    }
                }
            }
        }
    }

    // Always return 200 — Meta retries on non-200
    HttpResponse::Ok().finish()
}

fn contact_name_for_message(change: &Change, msg: &InboundMessage) -> Option<String> {
    change
        .value
        .contacts
        .as_ref()
        .and_then(|contacts| {
            contacts
                .iter()
                .find(|contact| contact.wa_id == msg.from)
                .or_else(|| contacts.first())
        })
        .and_then(|contact| contact.profile.as_ref())
        .map(|profile| profile.name.clone())
}

fn contact_phone_for_message(change: &Change, msg: &InboundMessage) -> String {
    change
        .value
        .contacts
        .as_ref()
        .and_then(|contacts| contacts.iter().find(|contact| contact.wa_id == msg.from))
        .map(|contact| contact.wa_id.clone())
        .unwrap_or_else(|| msg.from.clone())
}

#[cfg(test)]
fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => ch,
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('.')
        .trim_matches('_')
        .to_string()
        .if_empty("file.bin")
}

#[cfg(test)]
trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

#[cfg(test)]
impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/webhook/whatsapp")
            .service(verify)
            .service(receive),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_filename_removes_path_and_special_chars() {
        assert_eq!(sanitize_filename("../hello world!.jpg"), "hello_world_.jpg");
        assert_eq!(sanitize_filename("////"), "file.bin");
    }

    #[test]
    fn contact_phone_prefers_matching_wa_id() {
        let change: Change = serde_json::from_value(serde_json::json!({
            "field": "messages",
            "value": {
                "metadata": {"display_phone_number":"1", "phone_number_id":"phone-id"},
                "contacts": [
                    {"wa_id": "111", "profile": {"name": "Wrong"}},
                    {"wa_id": "222", "profile": {"name": "Right"}}
                ],
                "messages": [{
                    "from": "222",
                    "id": "wamid-1",
                    "timestamp": "1700000000",
                    "type": "text",
                    "text": {"body": "hi"}
                }]
            }
        }))
        .unwrap();
        let msg = change.value.messages.as_ref().unwrap().first().unwrap();

        assert_eq!(contact_phone_for_message(&change, msg), "222");
        assert_eq!(
            contact_name_for_message(&change, msg),
            Some("Right".to_string())
        );
    }
}
