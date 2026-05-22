use actix_web::{web, HttpResponse, get, post};
use sea_orm::{
    DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait,
    QueryOrder, PaginatorTrait, QuerySelect, Condition,
};
use serde::{Deserialize, Serialize};
use utoipa::{ToSchema, IntoParams};

use crate::config::AppConfig;
use crate::models::{conversation, message};
use crate::whatsapp::sender::WhatsAppSender;

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
    /// Public URL of the media
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
    db: web::Data<DatabaseConnection>,
    query: web::Query<ListConversationsQuery>,
) -> HttpResponse {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    let mut condition = Condition::all();
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

    let data: Vec<ConversationResponse> = conversations.into_iter().map(|c| ConversationResponse {
        id: c.id,
        contact_phone: c.contact_phone,
        contact_name: c.contact_name,
        last_message_at: c.last_message_at.map(|d| d.to_string()),
        created_at: c.created_at.to_string(),
    }).collect();

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
    db: web::Data<DatabaseConnection>,
    phone: web::Path<String>,
    query: web::Query<ListMessagesQuery>,
) -> HttpResponse {
    let conv = conversation::Entity::find()
        .filter(conversation::Column::ContactPhone.eq(phone.as_str()))
        .one(db.get_ref())
        .await
        .unwrap_or(None);

    let conv = match conv {
        Some(c) => c,
        None => return HttpResponse::NotFound().json(serde_json::json!({
            "success": false, "error": "Conversation not found"
        })),
    };

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).min(100);

    let total = message::Entity::find()
        .filter(message::Column::ConversationId.eq(conv.id))
        .count(db.get_ref())
        .await
        .unwrap_or(0);

    let messages = message::Entity::find()
        .filter(message::Column::ConversationId.eq(conv.id))
        .order_by_asc(message::Column::Timestamp)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(db.get_ref())
        .await
        .unwrap_or_default();

    let data: Vec<MessageResponse> = messages.into_iter().map(|m| MessageResponse {
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
    }).collect();

    HttpResponse::Ok().json(PaginatedMessages {
        success: true,
        data,
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
    db: web::Data<DatabaseConnection>,
    query: web::Query<SearchMessagesQuery>,
) -> HttpResponse {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    let mut condition = Condition::all()
        .add(message::Column::Body.contains(&query.q));

    // If phone filter, find conversation first
    if let Some(ref phone) = query.phone {
        let conv = conversation::Entity::find()
            .filter(conversation::Column::ContactPhone.eq(phone.as_str()))
            .one(db.get_ref())
            .await
            .unwrap_or(None);
        if let Some(c) = conv {
            condition = condition.add(message::Column::ConversationId.eq(c.id));
        } else {
            return HttpResponse::Ok().json(PaginatedMessages {
                success: true, data: vec![], page, per_page, total: 0,
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

    let data: Vec<MessageResponse> = messages.into_iter().map(|m| MessageResponse {
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
    }).collect();

    HttpResponse::Ok().json(PaginatedMessages {
        success: true,
        data,
        page,
        per_page,
        total,
    })
}

/// Send a text message
#[utoipa::path(
    post,
    path = "/api/chat/send/text",
    request_body = SendTextBody,
    responses(
        (status = 200, description = "Message sent", body = SendResponse),
    ),
    tag = "Chat"
)]
#[post("/send/text")]
pub async fn send_text(
    config: web::Data<AppConfig>,
    body: web::Json<SendTextBody>,
) -> HttpResponse {
    let sender = WhatsAppSender::new(&config);
    match sender.send_text(&body.to, &body.message).await {
        Ok(id) => HttpResponse::Ok().json(SendResponse {
            success: true, wa_message_id: Some(id), error: None,
        }),
        Err(e) => HttpResponse::Ok().json(SendResponse {
            success: false, wa_message_id: None, error: Some(e),
        }),
    }
}

/// Send a template message
#[utoipa::path(
    post,
    path = "/api/chat/send/template",
    request_body = SendTemplateBody,
    responses(
        (status = 200, description = "Template sent", body = SendResponse),
    ),
    tag = "Chat"
)]
#[post("/send/template")]
pub async fn send_template(
    config: web::Data<AppConfig>,
    body: web::Json<SendTemplateBody>,
) -> HttpResponse {
    let sender = WhatsAppSender::new(&config);
    match sender.send_template(&body.to, &body.template_name, &body.language, None).await {
        Ok(id) => HttpResponse::Ok().json(SendResponse {
            success: true, wa_message_id: Some(id), error: None,
        }),
        Err(e) => HttpResponse::Ok().json(SendResponse {
            success: false, wa_message_id: None, error: Some(e),
        }),
    }
}

/// Send a media message (image/document/audio/video)
#[utoipa::path(
    post,
    path = "/api/chat/send/media",
    request_body = SendMediaBody,
    responses(
        (status = 200, description = "Media sent", body = SendResponse),
    ),
    tag = "Chat"
)]
#[post("/send/media")]
pub async fn send_media(
    config: web::Data<AppConfig>,
    body: web::Json<SendMediaBody>,
) -> HttpResponse {
    let sender = WhatsAppSender::new(&config);
    match sender.send_media(&body.to, &body.media_type, &body.url, body.caption.as_deref()).await {
        Ok(id) => HttpResponse::Ok().json(SendResponse {
            success: true, wa_message_id: Some(id), error: None,
        }),
        Err(e) => HttpResponse::Ok().json(SendResponse {
            success: false, wa_message_id: None, error: Some(e),
        }),
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
            .service(send_media),
    );
}
