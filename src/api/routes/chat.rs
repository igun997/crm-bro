use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpResponse};
use bytes::Bytes;
use futures_util::StreamExt;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::api::middleware::CurrentUser;
use crate::application::auth::require_permission;
use crate::application::messaging::{
    active_whatsapp_account_id, cleanup_message, create_outbox, create_queued_message,
    ensure_contact_conversation, get_messages_by_phone as get_messages_by_phone_use_case,
    list_conversations as list_conversations_use_case, queue_send as queue_send_use_case,
    search_messages as search_messages_use_case, ListConversationsInput, ListMessagesInput,
    QueueSendInput, SearchMessagesInput,
};
use crate::domain::auth::permissions;
use crate::infrastructure::persistence::models::{conversation, message};
use crate::infrastructure::storage::StorageService;

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
#[schema(example = json!({"to": "628123456789", "message": "Hello from CRM Bro!"}))]
pub struct SendTextBody {
    /// Recipient phone number (with country code, e.g. 628xxx)
    pub to: String,
    /// Message text
    pub message: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"to": "628123456789", "template_name": "hello_world", "language": "en_US"}))]
pub struct SendTemplateBody {
    /// Recipient phone number
    pub to: String,
    /// Template name
    pub template_name: String,
    /// Language code (e.g. en_US, id)
    pub language: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"to": "628123456789", "media_type": "image", "url": "https://example.com/photo.jpg", "caption": "Check this out"}))]
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
#[schema(example = json!({"id": 1, "contact_phone": "628123456789", "contact_name": "Alice", "last_message_at": "2026-05-24T10:00:00", "created_at": "2026-05-20T08:00:00"}))]
pub struct ConversationResponse {
    pub id: i32,
    pub contact_phone: String,
    pub contact_name: Option<String>,
    pub last_message_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "id": 1, "conversation_id": 1, "wa_message_id": "wamid.abc123",
    "direction": "inbound", "msg_type": "text", "body": "Hello!",
    "media_url": null, "media_mime": null, "template_name": null,
    "status": "received", "timestamp": "2026-05-24T10:00:00"
}))]
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
#[schema(example = json!({"success": true, "data": [], "page": 1, "per_page": 20, "total": 0}))]
pub struct PaginatedConversations {
    pub success: bool,
    pub data: Vec<ConversationResponse>,
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"success": true, "data": [], "page": 1, "per_page": 20, "total": 0}))]
pub struct PaginatedMessages {
    pub success: bool,
    pub data: Vec<MessageResponse>,
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"success": true, "message_id": 42, "outbox_id": 7, "status": "queued", "wa_message_id": null, "error": null}))]
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
    if let Err(response) = require_permission(&ctx, permissions::CHATS_READ) {
        return response;
    }
    let tenant_id = match require_tenant(&ctx) {
        Ok(tenant_id) => tenant_id,
        Err(response) => return response,
    };

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    match list_conversations_use_case(
        db.get_ref(),
        ListConversationsInput {
            tenant_id,
            phone: query.phone.clone(),
            name: query.name.clone(),
            page,
            per_page,
        },
    )
    .await
    {
        Ok(output) => HttpResponse::Ok().json(PaginatedConversations {
            success: true,
            data: output
                .conversations
                .into_iter()
                .map(conversation_response)
                .collect(),
            page: output.page,
            per_page: output.per_page,
            total: output.total,
        }),
        Err(error) => send_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to list conversations: {error}"),
        ),
    }
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
    if let Err(response) = require_permission(&ctx, permissions::CHATS_READ) {
        return response;
    }
    let tenant_id = match require_tenant(&ctx) {
        Ok(tenant_id) => tenant_id,
        Err(response) => return response,
    };

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).min(100);

    match get_messages_by_phone_use_case(
        db.get_ref(),
        ListMessagesInput {
            tenant_id,
            phone: phone.into_inner(),
            page,
            per_page,
        },
    )
    .await
    {
        Ok(Some(output)) => HttpResponse::Ok().json(PaginatedMessages {
            success: true,
            data: output.messages.into_iter().map(message_response).collect(),
            page: output.page,
            per_page: output.per_page,
            total: output.total,
        }),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "success": false, "error": "Conversation not found"
        })),
        Err(error) => send_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to list messages: {error}"),
        ),
    }
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
    if let Err(response) = require_permission(&ctx, permissions::CHATS_READ) {
        return response;
    }
    let tenant_id = match require_tenant(&ctx) {
        Ok(tenant_id) => tenant_id,
        Err(response) => return response,
    };

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    match search_messages_use_case(
        db.get_ref(),
        SearchMessagesInput {
            tenant_id,
            q: query.q.clone(),
            phone: query.phone.clone(),
            page,
            per_page,
        },
    )
    .await
    {
        Ok(output) => HttpResponse::Ok().json(PaginatedMessages {
            success: true,
            data: output.messages.into_iter().map(message_response).collect(),
            page: output.page,
            per_page: output.per_page,
            total: output.total,
        }),
        Err(error) => send_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to search messages: {error}"),
        ),
    }
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
#[utoipa::path(
    post,
    path = "/api/chat/send/upload",
    request_body(
        content_type = "multipart/form-data",
        description = "Multipart upload with fields: to (phone), file (binary), caption (optional)"
    ),
    responses(
        (status = 200, description = "Upload queued", body = SendResponse),
        (status = 400, description = "Invalid multipart upload"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    tag = "Chat"
)]
#[post("/send/upload")]
pub async fn send_upload(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    storage: web::Data<StorageService>,
    mut payload: Multipart,
) -> HttpResponse {
    let ctx = current.0;
    if let Err(response) = require_permission(&ctx, permissions::CHATS_SEND) {
        return response;
    }
    let tenant_id = match require_tenant(&ctx) {
        Ok(tenant_id) => tenant_id,
        Err(response) => return response,
    };

    // Resolve tenant-specific storage
    let tenant_storage = match StorageService::resolve_for_tenant(db.get_ref(), tenant_id).await {
        Ok(ts) => ts,
        Err(error) => {
            tracing::error!(%error, "Failed to resolve tenant storage");
            None
        }
    };
    let effective_storage = tenant_storage.as_ref().unwrap_or(storage.get_ref());

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
                    Err(error) => {
                        return send_error(actix_web::http::StatusCode::BAD_REQUEST, &error)
                    }
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
                        return send_error(
                            actix_web::http::StatusCode::PAYLOAD_TOO_LARGE,
                            "Upload too large",
                        );
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
        _ => {
            return send_error(
                actix_web::http::StatusCode::BAD_REQUEST,
                "Missing 'to' field",
            )
        }
    };
    let file_bytes = match file_bytes {
        Some(bytes) => bytes,
        None => {
            return send_error(
                actix_web::http::StatusCode::BAD_REQUEST,
                "Missing 'file' field",
            )
        }
    };

    let account_id = match active_whatsapp_account_id(db.get_ref(), tenant_id).await {
        Ok(account_id) => account_id,
        Err(error) => return send_error(actix_web::http::StatusCode::BAD_REQUEST, &error),
    };

    let (contact, conv) = match ensure_contact_conversation(db.get_ref(), tenant_id, &phone).await {
        Ok(pair) => pair,
        Err(error) => {
            return send_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, &error)
        }
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
        Err(error) => {
            return send_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, &error)
        }
    };

    let key = format!(
        "tenants/{tenant_id}/contacts/{}/messages/{}/{}",
        contact.id,
        msg.id,
        sanitize_filename(&filename)
    );
    let stored = match effective_storage
        .put(&key, Bytes::from(file_bytes), &mime_type)
        .await
    {
        Ok(stored) => stored,
        Err(error) => {
            cleanup_message(db.get_ref(), msg.id).await;
            return send_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                &error.to_string(),
            );
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
            return send_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                &error.to_string(),
            );
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

#[allow(clippy::too_many_arguments)]
async fn queue_send(
    current: CurrentUser,
    db: &DatabaseConnection,
    phone: &str,
    msg_type: &str,
    body: Option<String>,
    template_name: Option<String>,
    media_url: Option<String>,
    payload: serde_json::Value,
) -> HttpResponse {
    let ctx = current.0;
    if let Err(response) = require_permission(&ctx, permissions::CHATS_SEND) {
        return response;
    }
    let tenant_id = match require_tenant(&ctx) {
        Ok(tenant_id) => tenant_id,
        Err(response) => return response,
    };

    match queue_send_use_case(
        db,
        QueueSendInput {
            tenant_id,
            phone: phone.to_string(),
            msg_type: msg_type.to_string(),
            body,
            template_name,
            media_url,
            payload,
        },
    )
    .await
    {
        Ok(output) => HttpResponse::Ok().json(SendResponse {
            success: true,
            message_id: Some(output.message_id),
            outbox_id: Some(output.outbox_id),
            status: Some("queued".to_string()),
            wa_message_id: None,
            error: None,
        }),
        Err(error) => send_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, &error),
    }
}

fn require_tenant(ctx: &crate::api::middleware::AuthContext) -> Result<i32, HttpResponse> {
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
