use actix_web::{web, HttpResponse, get, post};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, Set, ActiveModelTrait};

use crate::config::AppConfig;
use crate::models::conversation;
use crate::models::message;
use super::types::*;
use super::media;

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
#[get("")]
pub async fn verify(
    query: web::Query<VerifyQuery>,
    config: web::Data<AppConfig>,
) -> HttpResponse {
    let mode = query.mode.as_deref().unwrap_or("");
    let token = query.verify_token.as_deref().unwrap_or("");
    let challenge = query.challenge.as_deref().unwrap_or("");

    if mode == "subscribe" && token == config.wa_verify_token {
        tracing::info!("Webhook verified");
        HttpResponse::Ok().body(challenge.to_string())
    } else {
        tracing::warn!("Webhook verification failed");
        HttpResponse::Forbidden().finish()
    }
}

/// Receive messages/status updates from Meta
#[post("")]
pub async fn receive(
    body: web::Json<WebhookPayload>,
    db: web::Data<DatabaseConnection>,
    config: web::Data<AppConfig>,
) -> HttpResponse {
    for entry in &body.entry {
        for change in &entry.changes {
            if change.field != "messages" {
                continue;
            }

            // Handle incoming messages
            if let Some(messages) = &change.value.messages {
                let contact_name = change.value.contacts
                    .as_ref()
                    .and_then(|c| c.first())
                    .and_then(|c| c.profile.as_ref())
                    .map(|p| p.name.clone());

                for msg in messages {
                    if let Err(e) = handle_inbound_message(db.get_ref(), &config, msg, &contact_name).await {
                        tracing::error!("Failed to handle message {}: {}", msg.id, e);
                    }
                }
            }

            // Handle status updates
            if let Some(statuses) = &change.value.statuses {
                for status in statuses {
                    if let Err(e) = handle_status_update(db.get_ref(), status).await {
                        tracing::error!("Failed to handle status {}: {}", status.id, e);
                    }
                }
            }
        }
    }

    // Always return 200 — Meta retries on non-200
    HttpResponse::Ok().finish()
}

async fn handle_inbound_message(
    db: &DatabaseConnection,
    config: &AppConfig,
    msg: &InboundMessage,
    contact_name: &Option<String>,
) -> Result<(), String> {
    // Find or create conversation
    let conv = find_or_create_conversation(db, &msg.from, contact_name).await?;

    // Extract body & media info (download if media)
    let (body, media_url, media_mime) = extract_and_download_media(config, msg, conv.id).await;

    // Parse timestamp
    let ts = msg.timestamp.parse::<i64>().unwrap_or(0);
    let timestamp = chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.naive_utc())
        .unwrap_or_else(|| chrono::Utc::now().naive_utc());

    // Store message
    let new_msg = message::ActiveModel {
        conversation_id: Set(conv.id),
        wa_message_id: Set(Some(msg.id.clone())),
        direction: Set("inbound".into()),
        msg_type: Set(msg.msg_type.clone()),
        body: Set(body),
        media_url: Set(media_url),
        media_mime: Set(media_mime),
        template_name: Set(None),
        status: Set("received".into()),
        timestamp: Set(timestamp),
        ..Default::default()
    };
    new_msg.insert(db).await.map_err(|e| format!("DB insert message: {}", e))?;

    // Update conversation last_message_at
    let mut conv_update: conversation::ActiveModel = conv.into();
    conv_update.last_message_at = Set(Some(timestamp));
    conv_update.update(db).await.map_err(|e| format!("DB update conversation: {}", e))?;

    tracing::info!("Stored inbound message {} from {}", msg.id, msg.from);
    Ok(())
}

async fn handle_status_update(
    db: &DatabaseConnection,
    status: &StatusUpdate,
) -> Result<(), String> {
    // Find message by wa_message_id and update status
    let existing = message::Entity::find()
        .filter(message::Column::WaMessageId.eq(&status.id))
        .one(db)
        .await
        .map_err(|e| format!("DB query: {}", e))?;

    if let Some(msg) = existing {
        let mut update: message::ActiveModel = msg.into();
        update.status = Set(status.status.clone());
        update.update(db).await.map_err(|e| format!("DB update status: {}", e))?;
        tracing::info!("Updated message {} status to {}", status.id, status.status);
    }

    Ok(())
}

async fn find_or_create_conversation(
    db: &DatabaseConnection,
    phone: &str,
    name: &Option<String>,
) -> Result<conversation::Model, String> {
    let existing = conversation::Entity::find()
        .filter(conversation::Column::ContactPhone.eq(phone))
        .one(db)
        .await
        .map_err(|e| format!("DB query: {}", e))?;

    if let Some(conv) = existing {
        return Ok(conv);
    }

    let new_conv = conversation::ActiveModel {
        contact_phone: Set(phone.into()),
        contact_name: Set(name.clone()),
        ..Default::default()
    };

    new_conv.insert(db).await.map_err(|e| format!("DB insert conversation: {}", e))
}

async fn extract_and_download_media(
    config: &AppConfig,
    msg: &InboundMessage,
    conversation_id: i32,
) -> (Option<String>, Option<String>, Option<String>) {
    match msg.msg_type.as_str() {
        "text" => (msg.text.as_ref().map(|t| t.body.clone()), None, None),
        "image" | "document" | "audio" | "video" => {
            let media_info = match msg.msg_type.as_str() {
                "image" => msg.image.as_ref(),
                "document" => msg.document.as_ref(),
                "audio" => msg.audio.as_ref(),
                "video" => msg.video.as_ref(),
                _ => None,
            };

            let caption = media_info.and_then(|m| m.caption.clone());
            let media_id = media_info.map(|m| m.id.clone()).unwrap_or_default();

            // Download media locally
            match media::download_and_save(config, &media_id, conversation_id).await {
                Ok(downloaded) => (
                    caption,
                    Some(downloaded.local_path),
                    Some(downloaded.mime_type),
                ),
                Err(e) => {
                    tracing::error!("Media download failed for {}: {}", media_id, e);
                    (caption, Some(format!("media_id:{}", media_id)), media_info.and_then(|m| m.mime_type.clone()))
                }
            }
        }
        _ => (None, None, None),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/webhook/whatsapp")
            .service(verify)
            .service(receive),
    );
}
