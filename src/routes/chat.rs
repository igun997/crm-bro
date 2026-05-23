use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpResponse};
use bytes::Bytes;
use futures_util::StreamExt;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::auth::extractor::CurrentUser;
use crate::models::{contact, conversation, message, outbox_message, tenant_whatsapp_account};
use crate::rbac::require_permission;
use crate::storage::StorageService;

const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

// === Request/Response types ===

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListConversationsQuery {
    /// Filter by phone number (partial match)
    pub phone: Option<String>,
    /// Filter by contact name (partial match)
    pub name: Option<String>,
    /// Page number (1-based)
    pub page: Option<u64>,
    /// Items per page
    pub per_page: Option<u64>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListMessagesQuery {
    /// Page number (1-based)
    pub page: Option<u64>,
    /// Items per page
    pub per_page: Option<u64>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct SearchMessagesQuery {
    /// Search text in message body
    pub q: String,
    /// Filter by phone number
    pub phone: Option<String>,
    /// Page number (1-based)
    pub page: Option<u64>,
    /// Items per page
    pub per_page: Option<u64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SendTextBody {
    /// Recipient phone number (with country code, e.g. 628xxx)
    pub to: String,
    /// Message text
    pub message: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SendTemplateBody {
    /// Recipient phone number
    pub to: String,
    /// Template name
    pub template_name: String,
    /// Language code (e.g. en_US, id)
    pub language: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SendMediaBody {
    /// Recipient phone number
    pub to: String,
    /// Media type: image, document, audio, video
    pub media_type: String,
    /// Public URL or storage URL of media
    pub url: String,
    /// Optional caption
    pub caption: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConversationResponse {
    pub id: i32,
    pub contact_phone: String,
    pub contact_name: Option<String>,
    pub last_message_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageResponse {
    pub id: i32,
    pub conversation_id: i32,
    pub wa_message_id: Option<String>,
    pub direction: String,
    pub msg_type: String,
    pub body: Option<String>,
    pub media_url: Option<String>,
    pub media_mime: Option<String>,
    pub template_name: Option<String>,
    pub status: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PaginatedConversations {
    pub success: bool,
    pub data: Vec<ConversationResponse>,
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PaginatedMessages {
    pub success: bool,
    pub data: Vec<MessageResponse>,
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SendResponse {
    pub success: bool,
    pub message_id: Option<i32>,
    pub outbox_id: Option<i32>,
    pub status: Option<String>,
    pub wa_message_id: Option<String>,
    pub error: Option<String>,
}

// === Handlers ===

/// List all conversations with optional filters
#[utoipa::path(
    get,
    path = "/api/chat/conversations",
    params(ListConversationsQuery),
    responses(
        (status = 200, description = "List of conversations", body = PaginatedConversations),
    ),
    tag = "Chat"
)]
#[get("/conversations")]
pub async fn list_conversations(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    query: web::Query<ListConversationsQuery>,
) -> HttpResponse {
    let ctx = current.0;
    if let Err(response) = require_permission(&ctx, "chats.read") {
        return response;
    }
    let tenant_id = match require_tenant(&ctx) {
        Ok(tenant_id) => tenant_id,
        Err(response) => return response,
    };

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    let mut condition = Condition::all().add(conversation::Column::TenantId.eq(tenant_id));
    if let Some(ref phone) = query.phone {
        condition = condition.add(conversation::Column::ContactPhone.contains(phone));
    }
    if let Some(ref name) = query.name {
        condition = condition.add(conversation::Column::ContactName.contains(name));
    }

    let total = conversation::Entity::find()
        .filter(condition.clone())
        .count(db.get_ref())
        .await
        .unwrap_or(0);

    let conversations = conversation::Entity::find()
        .filter(condition)
        .order_by_desc(conversation::Column::LastMessageAt)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(db.get_ref())
        .await
        .unwrap_or_default();

    let data = conversations.into_iter().map(conversation_response).collect();

    HttpResponse::Ok().json(PaginatedConversations {
        success: true,
        data,
        page,
        per_page,
        total,
    })
}

/// Get messages for a conversation by phone number
#[utoipa::path(
    get,
    path = "/api/chat/messages/{phone}",
    params(
        ("phone" = String, Path, description = "Contact phone number"),
        ListMessagesQuery,
    ),
    responses(
        (status = 200, description = "Messages for conversation", body = PaginatedMessages),
        (status = 404, description = "Conversation not found"),
    ),
    tag = "Chat"
)]
#[get("/messages/{phone}")]
pub async fn get_messages_by_phone(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    phone: web::Path<String>,
    query: web::Query<ListMessagesQuery>,
) -> HttpResponse {
    let ctx = current.0;
    if let Err(response) = require_permission(&ctx, "chats.read") {
        return response;
    }
    let tenant_id = match require_tenant(&ctx) {
        Ok(tenant_id) => tenant_id,
        Err(response) => return response,
    };

    let conv = conversation::Entity::find()
        .filter(conversation::Column::TenantId.eq(tenant_id))
        .filter(conversation::Column::ContactPhone.eq(phone.as_str()))
        .one(db.get_ref())
        .await
        .unwrap_or(None);

    let conv = match conv {
        Some(c) => c,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "success": false, "error": "Conversation not found"
            }));
        }
    };

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).min(100);

    let condition = Condition::all()
        .add(message::Column::TenantId.eq(tenant_id))
        .add(message::Column::ConversationId.eq(conv.id));

    let total = message::Entity::find()
        .filter(condition.clone())
        .count(db.get_ref())
        .await
        .unwrap_or(0);

    let messages = message::Entity::find()
        .filter(condition)
        .order_by_asc(message::Column::Timestamp)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(db.get_ref())
        .await
        .unwrap_or_default();

    HttpResponse::Ok().json(PaginatedMessages {
        success: true,
        data: messages.into_iter().map(message_response).collect(),
        page,
        per_page,
        total,
    })
}

/// Search messages across all conversations
#[utoipa::path(
    get,
    path = "/api/chat/search",
    params(SearchMessagesQuery),
    responses(
        (status = 200, description = "Search results", body = PaginatedMessages),
    ),
    tag = "Chat"
)]
#[get("/search")]
pub async fn search_messages(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    query: web::Query<SearchMessagesQuery>,
) -> HttpResponse {
    let ctx = current.0;
    if let Err(response) = require_permission(&ctx, "chats.read") {
        return response;
    }
    let tenant_id = match require_tenant(&ctx) {
        Ok(tenant_id) => tenant_id,
        Err(response) => return response,
    };

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    let mut condition = Condition::all()
        .add(message::Column::TenantId.eq(tenant_id))
        .add(message::Column::Body.contains(&query.q));

    if let Some(ref phone) = query.phone {
        let conv = conversation::Entity::find()
            .filter(conversation::Column::TenantId.eq(tenant_id))
            .filter(conversation::Column::ContactPhone.eq(phone.as_str()))
            .one(db.get_ref())
            .await
            .unwrap_or(None);
        if let Some(c) = conv {
            condition = condition.add(message::Column::ConversationId.eq(c.id));
        } else {
            return HttpResponse::Ok().json(PaginatedMessages {
                success: true,
                data: vec![],
                page,
                per_page,
                total: 0,
            });
        }
    }

    let total = message::Entity::find()
        .filter(condition.clone())
        .count(db.get_ref())
        .await
        .unwrap_or(0);

    let messages = message::Entity::find()
        .filter(condition)
        .order_by_desc(message::Column::Timestamp)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(db.get_ref())
        .await
        .unwrap_or_default();

    HttpResponse::Ok().json(PaginatedMessages {
        success: true,
        data: messages.into_iter().map(message_response).collect(),
        page,
        per_page,
        total,
    })
}

/// Queue a text message
#[utoipa::path(
    post,
    path = "/api/chat/send/text",
    request_body = SendTextBody,
    responses(
        (status = 200, description = "Message queued", body = SendResponse),
    ),
    tag = "Chat"
)]
#[post("/send/text")]
pub async fn send_text(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    body: web::Json<SendTextBody>,
) -> HttpResponse {
    queue_send(
        current,
        db.get_ref(),
        &body.to,
        "text",
        Some(body.message.clone()),
        None,
        None,
        serde_json::json!({"type":"text","to":body.to,"message":body.message}),
    )
    .await
}

/// Queue a template message
#[utoipa::path(
    post,
    path = "/api/chat/send/template",
    request_body = SendTemplateBody,
    responses(
        (status = 200, description = "Template queued", body = SendResponse),
    ),
    tag = "Chat"
)]
#[post("/send/template")]
pub async fn send_template(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    body: web::Json<SendTemplateBody>,
) -> HttpResponse {
    queue_send(
        current,
        db.get_ref(),
        &body.to,
        "template",
        None,
        Some(body.template_name.clone()),
        None,
        serde_json::json!({"type":"template","to":body.to,"template_name":body.template_name,"language":body.language}),
    )
    .await
}

/// Queue a media message (image/document/audio/video)
#[utoipa::path(
    post,
    path = "/api/chat/send/media",
    request_body = SendMediaBody,
    responses(
        (status = 200, description = "Media queued", body = SendResponse),
    ),
    tag = "Chat"
)]
#[post("/send/media")]
pub async fn send_media(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    body: web::Json<SendMediaBody>,
) -> HttpResponse {
    queue_send(
        current,
        db.get_ref(),
        &body.to,
        &body.media_type,
        body.caption.clone(),
        None,
        Some(body.url.clone()),
        serde_json::json!({"type":"media","to":body.to,"media_type":body.media_type,"url":body.url,"caption":body.caption}),
    )
    .await
}

/// Upload and queue a file via WhatsApp
#[post("/send/upload")]
pub async fn send_upload(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    storage: web::Data<StorageService>,
    mut payload: Multipart,
) -> HttpResponse {
    let ctx = current.0;
    if let Err(response) = require_permission(&ctx, "chats.send") {
        return response;
    }
    let tenant_id = match require_tenant(&ctx) {
        Ok(tenant_id) => tenant_id,
        Err(response) => return response,
    };

    let mut phone: Option<String> = None;
    let mut caption: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename = "upload".to_string();
    let mut mime_type = "application/octet-stream".to_string();
    let mut media_type = "document".to_string();

    while let Some(field_result) = payload.next().await {
        let mut field = match field_result {
            Ok(field) => field,
            Err(error) => {
                return send_error(
                    actix_web::http::StatusCode::BAD_REQUEST,
                    &format!("Invalid multipart field: {error}"),
                );
            }
        };
        let name = field.name().map(|n| n.to_string()).unwrap_or_default();
        match name.as_str() {
            "to" => match read_text_field(&mut field).await {
                Ok(value) => phone = Some(value),
                Err(error) => return send_error(actix_web::http::StatusCode::BAD_REQUEST, &error),
            },
            "caption" => {
                let value = match read_text_field(&mut field).await {
                    Ok(value) => value,
                    Err(error) => return send_error(actix_web::http::StatusCode::BAD_REQUEST, &error),
                };
                if !value.is_empty() {
                    caption = Some(value);
                }
            }
            "file" => {
                if let Some(ct) = field.content_type() {
                    mime_type = ct.to_string();
                }
                media_type = media_type_from_mime(&mime_type).to_string();
                filename = field
                    .content_disposition()
                    .and_then(|cd| cd.get_filename().map(|s| s.to_string()))
                    .unwrap_or_else(|| "upload".to_string());
                let mut data = Vec::new();
                while let Some(chunk_result) = field.next().await {
                    let chunk = match chunk_result {
                        Ok(chunk) => chunk,
                        Err(error) => {
                            return send_error(
                                actix_web::http::StatusCode::BAD_REQUEST,
                                &format!("Read upload: {error}"),
                            );
                        }
                    };
                    if data.len() + chunk.len() > MAX_UPLOAD_BYTES {
                        return send_error(actix_web::http::StatusCode::PAYLOAD_TOO_LARGE, "Upload too large");
                    }
                    data.extend_from_slice(&chunk);
                }
                file_bytes = Some(data);
            }
            _ => {}
        }
    }

    let phone = match phone {
        Some(p) if !p.trim().is_empty() => p,
        _ => return send_error(actix_web::http::StatusCode::BAD_REQUEST, "Missing 'to' field"),
    };
    let file_bytes = match file_bytes {
        Some(bytes) => bytes,
        None => return send_error(actix_web::http::StatusCode::BAD_REQUEST, "Missing 'file' field"),
    };

    let account_id = match active_whatsapp_account_id(db.get_ref(), tenant_id).await {
        Ok(account_id) => account_id,
        Err(error) => return send_error(actix_web::http::StatusCode::BAD_REQUEST, &error),
    };

    let (contact, conv) = match ensure_contact_conversation(db.get_ref(), tenant_id, &phone).await {
        Ok(pair) => pair,
        Err(error) => return send_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, &error),
    };

    let now = chrono::Utc::now().naive_utc();
    let msg = match create_queued_message(
        db.get_ref(),
        tenant_id,
        contact.id,
        conv.id,
        &media_type,
        caption.clone(),
        None,
        None,
        None,
        Some(filename.clone()),
        Some(file_bytes.len() as i64),
        now,
    )
    .await
    {
        Ok(msg) => msg,
        Err(error) => return send_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, &error),
    };

    let key = format!(
        "tenants/{tenant_id}/contacts/{}/messages/{}/{}",
        contact.id,
        msg.id,
        sanitize_filename(&filename)
    );
    let stored = match storage.put(&key, Bytes::from(file_bytes), &mime_type).await {
        Ok(stored) => stored,
        Err(error) => {
            cleanup_message(db.get_ref(), msg.id).await;
            return send_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, &error);
        }
    };

    let msg_id = msg.id;
    let mut msg_update: message::ActiveModel = msg.into();
    msg_update.storage_key = Set(Some(stored.key.clone()));
    msg_update.media_url = Set(Some(stored.url.clone()));
    msg_update.media_mime = Set(Some(mime_type.clone()));
    let msg = match msg_update.update(db.get_ref()).await {
        Ok(msg) => msg,
        Err(error) => {
            cleanup_message(db.get_ref(), msg_id).await;
            return send_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                &format!("DB update uploaded message: {error}"),
            );
        }
    };

    let outbox = match create_outbox(
        db.get_ref(),
        tenant_id,
        msg.id,
        "send_media",
        serde_json::json!({
            "type":"media",
            "to":phone,
            "media_type":media_type,
            "storage_key":stored.key,
            "url":stored.url,
            "caption":caption,
            "mime_type":mime_type,
            "filename":filename,
            "whatsapp_account_id":account_id,
        }),
    )
    .await
    {
        Ok(outbox) => outbox,
        Err(error) => {
            cleanup_message(db.get_ref(), msg.id).await;
            return send_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, &error);
        }
    };

    HttpResponse::Ok().json(SendResponse {
        success: true,
        message_id: Some(msg.id),
        outbox_id: Some(outbox.id),
        status: Some("queued".to_string()),
        wa_message_id: None,
        error: None,
    })
}

async fn queue_send(
    current: CurrentUser,
    db: &DatabaseConnection,
    phone: &str,
    msg_type: &str,
    body: Option<String>,
    template_name: Option<String>,
    media_url: Option<String>,
    mut payload: serde_json::Value,
) -> HttpResponse {
    let ctx = current.0;
    if let Err(response) = require_permission(&ctx, "chats.send") {
        return response;
    }
    let tenant_id = match require_tenant(&ctx) {
        Ok(tenant_id) => tenant_id,
        Err(response) => return response,
    };

    let account_id = match active_whatsapp_account_id(db, tenant_id).await {
        Ok(account_id) => account_id,
        Err(error) => return send_error(actix_web::http::StatusCode::BAD_REQUEST, &error),
    };
    let (contact, conv) = match ensure_contact_conversation(db, tenant_id, phone).await {
        Ok(pair) => pair,
        Err(error) => return send_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, &error),
    };
    payload["whatsapp_account_id"] = serde_json::json!(account_id);

    let now = chrono::Utc::now().naive_utc();
    let msg = match create_queued_message(
        db,
        tenant_id,
        contact.id,
        conv.id,
        msg_type,
        body,
        template_name,
        media_url,
        None,
        None,
        None,
        now,
    )
    .await
    {
        Ok(msg) => msg,
        Err(error) => return send_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, &error),
    };

    let kind = if msg_type == "template" {
        "send_template"
    } else if msg_type == "text" {
        "send_text"
    } else {
        "send_media"
    };
    let outbox = match create_outbox(db, tenant_id, msg.id, kind, payload).await {
        Ok(outbox) => outbox,
        Err(error) => {
            cleanup_message(db, msg.id).await;
            return send_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, &error);
        }
    };

    HttpResponse::Ok().json(SendResponse {
        success: true,
        message_id: Some(msg.id),
        outbox_id: Some(outbox.id),
        status: Some("queued".to_string()),
        wa_message_id: None,
        error: None,
    })
}

async fn ensure_contact_conversation(
    db: &DatabaseConnection,
    tenant_id: i32,
    phone: &str,
) -> Result<(contact::Model, conversation::Model), String> {
    let contact = match contact::Entity::find()
        .filter(contact::Column::TenantId.eq(tenant_id))
        .filter(contact::Column::Phone.eq(phone))
        .one(db)
        .await
        .map_err(|error| format!("DB query contact: {error}"))?
    {
        Some(contact) => contact,
        None => contact::ActiveModel {
            tenant_id: Set(tenant_id),
            phone: Set(phone.to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|error| format!("DB insert contact: {error}"))?,
    };

    let conv = match conversation::Entity::find()
        .filter(conversation::Column::TenantId.eq(tenant_id))
        .filter(conversation::Column::ContactId.eq(contact.id))
        .one(db)
        .await
        .map_err(|error| format!("DB query conversation: {error}"))?
    {
        Some(conv) => conv,
        None => conversation::ActiveModel {
            tenant_id: Set(Some(tenant_id)),
            contact_id: Set(Some(contact.id)),
            contact_phone: Set(phone.to_string()),
            contact_name: Set(contact.name.clone()),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|error| format!("DB insert conversation: {error}"))?,
    };

    Ok((contact, conv))
}

async fn cleanup_message(db: &DatabaseConnection, message_id: i32) {
    if let Err(error) = message::Entity::delete_by_id(message_id).exec(db).await {
        tracing::error!(%error, message_id, "Failed to clean up queued message");
    }
}

async fn active_whatsapp_account_id(db: &DatabaseConnection, tenant_id: i32) -> Result<i32, String> {
    tenant_whatsapp_account::Entity::find()
        .filter(tenant_whatsapp_account::Column::TenantId.eq(tenant_id))
        .filter(tenant_whatsapp_account::Column::IsActive.eq(true))
        .one(db)
        .await
        .map_err(|error| format!("DB query WhatsApp account: {error}"))?
        .map(|account| account.id)
        .ok_or_else(|| "No active WhatsApp account configured".to_string())
}

async fn create_queued_message(
    db: &DatabaseConnection,
    tenant_id: i32,
    contact_id: i32,
    conversation_id: i32,
    msg_type: &str,
    body: Option<String>,
    template_name: Option<String>,
    media_url: Option<String>,
    storage_key: Option<String>,
    original_filename: Option<String>,
    size_bytes: Option<i64>,
    now: chrono::NaiveDateTime,
) -> Result<message::Model, String> {
    let msg = message::ActiveModel {
        conversation_id: Set(conversation_id),
        wa_message_id: Set(None),
        direction: Set("outbound".to_string()),
        msg_type: Set(msg_type.to_string()),
        body: Set(body),
        media_url: Set(media_url),
        media_mime: Set(None),
        template_name: Set(template_name),
        status: Set("queued".to_string()),
        timestamp: Set(now),
        tenant_id: Set(Some(tenant_id)),
        contact_id: Set(Some(contact_id)),
        storage_key: Set(storage_key),
        original_filename: Set(original_filename),
        size_bytes: Set(size_bytes),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|error| format!("DB insert queued message: {error}"))?;

    if let Ok(Some(conv)) = conversation::Entity::find_by_id(conversation_id).one(db).await {
        let mut update: conversation::ActiveModel = conv.into();
        update.last_message_at = Set(Some(now));
        let _ = update.update(db).await;
    }

    Ok(msg)
}

async fn create_outbox(
    db: &DatabaseConnection,
    tenant_id: i32,
    message_id: i32,
    kind: &str,
    payload_json: serde_json::Value,
) -> Result<outbox_message::Model, String> {
    outbox_message::ActiveModel {
        tenant_id: Set(tenant_id),
        message_id: Set(message_id),
        kind: Set(kind.to_string()),
        payload_json: Set(payload_json),
        status: Set("pending".to_string()),
        attempts: Set(0),
        next_attempt_at: Set(chrono::Utc::now().naive_utc()),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|error| format!("DB insert outbox: {error}"))
}

fn require_tenant(ctx: &crate::auth::context::AuthContext) -> Result<i32, HttpResponse> {
    ctx.tenant_id.ok_or_else(|| {
        HttpResponse::Forbidden().json(serde_json::json!({
            "success": false,
            "error": "Tenant context required"
        }))
    })
}

fn send_error(status: actix_web::http::StatusCode, error: &str) -> HttpResponse {
    HttpResponse::build(status).json(SendResponse {
        success: false,
        message_id: None,
        outbox_id: None,
        status: None,
        wa_message_id: None,
        error: Some(error.to_string()),
    })
}

fn conversation_response(c: conversation::Model) -> ConversationResponse {
    ConversationResponse {
        id: c.id,
        contact_phone: c.contact_phone,
        contact_name: c.contact_name,
        last_message_at: c.last_message_at.map(|d| d.to_string()),
        created_at: c.created_at.to_string(),
    }
}

fn message_response(m: message::Model) -> MessageResponse {
    MessageResponse {
        id: m.id,
        conversation_id: m.conversation_id,
        wa_message_id: m.wa_message_id,
        direction: m.direction,
        msg_type: m.msg_type,
        body: m.body,
        media_url: m.media_url,
        media_mime: m.media_mime,
        template_name: m.template_name,
        status: m.status,
        timestamp: m.timestamp.to_string(),
    }
}

async fn read_text_field(field: &mut actix_multipart::Field) -> Result<String, String> {
    let mut data = Vec::new();
    while let Some(chunk_result) = field.next().await {
        let chunk = chunk_result.map_err(|error| format!("Read form field: {error}"))?;
        if data.len() + chunk.len() > MAX_UPLOAD_BYTES {
            return Err("Form field too large".to_string());
        }
        data.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&data).to_string())
}

fn media_type_from_mime(mime_type: &str) -> &str {
    if mime_type.starts_with("image") {
        "image"
    } else if mime_type.starts_with("video") {
        "video"
    } else if mime_type.starts_with("audio") {
        "audio"
    } else {
        "document"
    }
}

fn sanitize_filename(filename: &str) -> String {
    let sanitized = filename
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => ch,
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('.')
        .trim_matches('_')
        .to_string();
    if sanitized.is_empty() {
        "file.bin".to_string()
    } else {
        sanitized
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/chat")
            .service(list_conversations)
            .service(get_messages_by_phone)
            .service(search_messages)
            .service(send_text)
            .service(send_template)
            .service(send_media)
            .service(send_upload),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_type_from_mime_maps_common_types() {
        assert_eq!(media_type_from_mime("image/png"), "image");
        assert_eq!(media_type_from_mime("video/mp4"), "video");
        assert_eq!(media_type_from_mime("audio/ogg"), "audio");
        assert_eq!(media_type_from_mime("application/pdf"), "document");
    }

    #[test]
    fn sanitize_filename_rejects_path_segments() {
        assert_eq!(sanitize_filename("../hello world!.jpg"), "hello_world_.jpg");
        assert_eq!(sanitize_filename("////"), "file.bin");
    }
}
