use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::domain::messaging::MessageStatus;
use crate::infrastructure::persistence::models::{
    contact, conversation, message, tenant_whatsapp_account,
};
use crate::infrastructure::storage::StorageService;
use crate::infrastructure::websocket::{ChatHub, ChatMessage as WsChatMessage};
use crate::infrastructure::whatsapp::media;
use crate::infrastructure::whatsapp::types::{Change, InboundMessage, MediaInfo, StatusUpdate};

pub async fn resolve_whatsapp_account(
    db: &DatabaseConnection,
    change: &Change,
) -> Result<Option<tenant_whatsapp_account::Model>, String> {
    let Some(metadata) = &change.value.metadata else {
        return Ok(None);
    };
    tenant_whatsapp_account::Entity::find()
        .filter(tenant_whatsapp_account::Column::PhoneNumberId.eq(&metadata.phone_number_id))
        .filter(tenant_whatsapp_account::Column::IsActive.eq(true))
        .one(db)
        .await
        .map_err(|error| format!("DB query tenant WhatsApp account: {error}"))
}

pub async fn handle_inbound_message(
    db: &DatabaseConnection,
    storage: &StorageService,
    hub: &actix::Addr<ChatHub>,
    account: &tenant_whatsapp_account::Model,
    msg: &InboundMessage,
    phone: &str,
    contact_name: &Option<String>,
) -> Result<(), String> {
    let contact = upsert_contact(db, account.tenant_id, phone, contact_name).await?;
    let conv = find_or_create_conversation(db, account, &contact, phone, contact_name).await?;
    let (body, media_meta) = extract_body_and_media(msg);
    let timestamp = parse_timestamp(&msg.timestamp);

    let new_msg = message::ActiveModel {
        conversation_id: Set(conv.id),
        wa_message_id: Set(Some(msg.id.clone())),
        direction: Set("inbound".into()),
        msg_type: Set(msg.msg_type.clone()),
        body: Set(body.clone()),
        media_url: Set(None),
        media_mime: Set(media_meta.as_ref().and_then(|m| m.mime_type.clone())),
        template_name: Set(None),
        status: Set("received".into()),
        timestamp: Set(timestamp),
        tenant_id: Set(Some(account.tenant_id)),
        contact_id: Set(Some(contact.id)),
        storage_key: Set(None),
        original_filename: Set(media_meta.as_ref().and_then(original_filename)),
        size_bytes: Set(None),
        ..Default::default()
    };
    let inserted = new_msg
        .insert(db)
        .await
        .map_err(|error| format!("DB insert message: {error}"))?;

    let inserted = if let Some(media_info) = media_meta {
        match store_message_media(
            db,
            storage,
            account,
            &contact,
            inserted.clone(),
            &media_info,
        )
        .await
        {
            Ok(updated) => updated,
            Err(error) => {
                tracing::error!(%error, message_id = inserted.id, "Failed to store inbound media");
                inserted
            }
        }
    } else {
        inserted
    };

    let conv_id = conv.id;
    let mut conv_update: conversation::ActiveModel = conv.into();
    conv_update.last_message_at = Set(Some(timestamp));
    conv_update.contact_name = Set(contact_name.clone());
    conv_update
        .update(db)
        .await
        .map_err(|error| format!("DB update conversation: {error}"))?;

    hub.do_send(WsChatMessage {
        tenant_id: account.tenant_id,
        conversation_id: conv_id,
        message_id: inserted.id,
        direction: "inbound".into(),
        msg_type: msg.msg_type.clone(),
        body,
        contact_phone: phone.to_string(),
        contact_name: contact_name.clone(),
        timestamp: timestamp.to_string(),
    });
    Ok(())
}

pub async fn handle_status_update(
    db: &DatabaseConnection,
    status: &StatusUpdate,
) -> Result<(), String> {
    let existing = message::Entity::find()
        .filter(message::Column::WaMessageId.eq(&status.id))
        .one(db)
        .await
        .map_err(|error| format!("DB query: {error}"))?;

    if let Some(msg) = existing {
        let parsed_status = MessageStatus::parse(&status.status)
            .map_err(|error| format!("Invalid WhatsApp status update: {error}"))?;
        if parsed_status == MessageStatus::Failed && msg.wa_message_id.is_some() {
            tracing::warn!(message_id = msg.id, wa_message_id = ?msg.wa_message_id, "Skipping failed status update because WhatsApp message id exists");
            return Ok(());
        }
        let mut update: message::ActiveModel = msg.into();
        update.status = Set(parsed_status.as_str().to_string());
        update
            .update(db)
            .await
            .map_err(|error| format!("DB update status: {error}"))?;
    }
    Ok(())
}

async fn store_message_media(
    db: &DatabaseConnection,
    storage: &StorageService,
    account: &tenant_whatsapp_account::Model,
    contact: &contact::Model,
    msg: message::Model,
    media_info: &MediaInfo,
) -> Result<message::Model, String> {
    let downloaded =
        media::download_bytes(&account.api_version, &account.access_token, &media_info.id).await?;
    let ext = media::mime_to_extension(&downloaded.mime_type);
    let filename = media_info.filename.clone().unwrap_or_else(|| {
        format!(
            "{}_{}.{}",
            media_info.id,
            chrono::Utc::now().timestamp(),
            ext
        )
    });
    let key = format!(
        "tenants/{}/contacts/{}/messages/{}/{}",
        account.tenant_id,
        contact.id,
        msg.id,
        sanitize_filename(&filename)
    );
    let stored = storage
        .put(&key, downloaded.bytes, &downloaded.mime_type)
        .await?;

    let mut update: message::ActiveModel = msg.into();
    update.storage_key = Set(Some(stored.key));
    update.media_url = Set(Some(stored.url));
    update.media_mime = Set(Some(downloaded.mime_type));
    update.original_filename = Set(Some(filename));
    update.size_bytes = Set(Some(stored.size_bytes as i64));
    update
        .update(db)
        .await
        .map_err(|error| format!("DB update message media: {error}"))
}

async fn upsert_contact(
    db: &DatabaseConnection,
    tenant_id: i32,
    phone: &str,
    name: &Option<String>,
) -> Result<contact::Model, String> {
    let existing = contact::Entity::find()
        .filter(contact::Column::TenantId.eq(tenant_id))
        .filter(contact::Column::Phone.eq(phone))
        .one(db)
        .await
        .map_err(|error| format!("DB query contact: {error}"))?;
    if let Some(contact) = existing {
        if contact
            .name
            .as_ref()
            .is_none_or(|existing| existing.trim().is_empty())
            && name.is_some()
        {
            let mut update: contact::ActiveModel = contact.into();
            update.name = Set(name.clone());
            return update
                .update(db)
                .await
                .map_err(|error| format!("DB update contact: {error}"));
        }
        return Ok(contact);
    }
    contact::ActiveModel {
        tenant_id: Set(tenant_id),
        phone: Set(phone.to_string()),
        name: Set(name.clone()),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|error| format!("DB insert contact: {error}"))
}

async fn find_or_create_conversation(
    db: &DatabaseConnection,
    account: &tenant_whatsapp_account::Model,
    contact: &contact::Model,
    phone: &str,
    name: &Option<String>,
) -> Result<conversation::Model, String> {
    let existing = conversation::Entity::find()
        .filter(conversation::Column::TenantId.eq(account.tenant_id))
        .filter(conversation::Column::ContactId.eq(contact.id))
        .one(db)
        .await
        .map_err(|error| format!("DB query conversation: {error}"))?;
    if let Some(conv) = existing {
        return Ok(conv);
    }
    conversation::ActiveModel {
        contact_phone: Set(phone.to_string()),
        contact_name: Set(name.clone()),
        tenant_id: Set(Some(account.tenant_id)),
        contact_id: Set(Some(contact.id)),
        whatsapp_account_id: Set(Some(account.id)),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|error| format!("DB insert conversation: {error}"))
}

fn parse_timestamp(timestamp: &str) -> chrono::NaiveDateTime {
    let ts = timestamp.parse::<i64>().unwrap_or(0);
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.naive_utc())
        .unwrap_or_else(|| chrono::Utc::now().naive_utc())
}

fn extract_body_and_media(msg: &InboundMessage) -> (Option<String>, Option<MediaInfo>) {
    match msg.msg_type.as_str() {
        "text" => (msg.text.as_ref().map(|text| text.body.clone()), None),
        "image" => media_body(msg.image.as_ref()),
        "document" => media_body(msg.document.as_ref()),
        "audio" => media_body(msg.audio.as_ref()),
        "video" => media_body(msg.video.as_ref()),
        _ => (None, None),
    }
}

fn media_body(media: Option<&MediaInfo>) -> (Option<String>, Option<MediaInfo>) {
    (
        media.and_then(|media| media.caption.clone()),
        media.cloned(),
    )
}
fn original_filename(media: &MediaInfo) -> Option<String> {
    media.filename.clone()
}
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
trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}
impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}
