use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};

use crate::infrastructure::persistence::models::{
    contact, conversation, message, outbox_message, tenant_whatsapp_account,
};

pub struct ListConversationsInput {
    pub tenant_id: i32,
    pub phone: Option<String>,
    pub name: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

pub struct ListConversationsOutput {
    pub conversations: Vec<conversation::Model>,
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
}

pub struct ListMessagesInput {
    pub tenant_id: i32,
    pub phone: String,
    pub page: u64,
    pub per_page: u64,
}

pub struct ListMessagesOutput {
    pub messages: Vec<message::Model>,
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
}

pub struct SearchMessagesInput {
    pub tenant_id: i32,
    pub q: String,
    pub phone: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

pub struct QueueSendInput {
    pub tenant_id: i32,
    pub phone: String,
    pub msg_type: String,
    pub body: Option<String>,
    pub template_name: Option<String>,
    pub media_url: Option<String>,
    pub payload: serde_json::Value,
}

pub struct QueueSendOutput {
    pub message_id: i32,
    pub outbox_id: i32,
}

pub async fn list_conversations(
    db: &DatabaseConnection,
    input: ListConversationsInput,
) -> Result<ListConversationsOutput, sea_orm::DbErr> {
    let mut condition = Condition::all().add(conversation::Column::TenantId.eq(input.tenant_id));
    if let Some(ref phone) = input.phone {
        condition = condition.add(conversation::Column::ContactPhone.contains(phone));
    }
    if let Some(ref name) = input.name {
        condition = condition.add(conversation::Column::ContactName.contains(name));
    }

    let total = conversation::Entity::find()
        .filter(condition.clone())
        .count(db)
        .await?;
    let conversations = conversation::Entity::find()
        .filter(condition)
        .order_by_desc(conversation::Column::LastMessageAt)
        .offset((input.page - 1) * input.per_page)
        .limit(input.per_page)
        .all(db)
        .await?;

    Ok(ListConversationsOutput {
        conversations,
        page: input.page,
        per_page: input.per_page,
        total,
    })
}

pub async fn get_messages_by_phone(
    db: &DatabaseConnection,
    input: ListMessagesInput,
) -> Result<Option<ListMessagesOutput>, sea_orm::DbErr> {
    let conv = conversation::Entity::find()
        .filter(conversation::Column::TenantId.eq(input.tenant_id))
        .filter(conversation::Column::ContactPhone.eq(input.phone.as_str()))
        .one(db)
        .await?;
    let Some(conv) = conv else {
        return Ok(None);
    };

    let condition = Condition::all()
        .add(message::Column::TenantId.eq(input.tenant_id))
        .add(message::Column::ConversationId.eq(conv.id));
    let total = message::Entity::find()
        .filter(condition.clone())
        .count(db)
        .await?;
    let messages = message::Entity::find()
        .filter(condition)
        .order_by_asc(message::Column::Timestamp)
        .offset((input.page - 1) * input.per_page)
        .limit(input.per_page)
        .all(db)
        .await?;

    Ok(Some(ListMessagesOutput {
        messages,
        page: input.page,
        per_page: input.per_page,
        total,
    }))
}

pub async fn search_messages(
    db: &DatabaseConnection,
    input: SearchMessagesInput,
) -> Result<ListMessagesOutput, sea_orm::DbErr> {
    let mut condition = Condition::all()
        .add(message::Column::TenantId.eq(input.tenant_id))
        .add(message::Column::Body.contains(&input.q));

    if let Some(ref phone) = input.phone {
        let conv = conversation::Entity::find()
            .filter(conversation::Column::TenantId.eq(input.tenant_id))
            .filter(conversation::Column::ContactPhone.eq(phone.as_str()))
            .one(db)
            .await?;
        if let Some(c) = conv {
            condition = condition.add(message::Column::ConversationId.eq(c.id));
        } else {
            return Ok(ListMessagesOutput {
                messages: vec![],
                page: input.page,
                per_page: input.per_page,
                total: 0,
            });
        }
    }

    let total = message::Entity::find()
        .filter(condition.clone())
        .count(db)
        .await?;
    let messages = message::Entity::find()
        .filter(condition)
        .order_by_desc(message::Column::Timestamp)
        .offset((input.page - 1) * input.per_page)
        .limit(input.per_page)
        .all(db)
        .await?;
    Ok(ListMessagesOutput {
        messages,
        page: input.page,
        per_page: input.per_page,
        total,
    })
}

pub async fn queue_send(
    db: &DatabaseConnection,
    mut input: QueueSendInput,
) -> Result<QueueSendOutput, String> {
    let account_id = active_whatsapp_account_id(db, input.tenant_id).await?;
    let (contact, conv) = ensure_contact_conversation(db, input.tenant_id, &input.phone).await?;
    input.payload["whatsapp_account_id"] = serde_json::json!(account_id);

    let now = chrono::Utc::now().naive_utc();
    let msg = create_queued_message(
        db,
        input.tenant_id,
        contact.id,
        conv.id,
        &input.msg_type,
        input.body,
        input.template_name,
        input.media_url,
        None,
        None,
        None,
        now,
    )
    .await?;

    let kind = if input.msg_type == "template" {
        "send_template"
    } else if input.msg_type == "text" {
        "send_text"
    } else {
        "send_media"
    };
    let outbox = create_outbox(db, input.tenant_id, msg.id, kind, input.payload)
        .await
        .map_err(|error| format!("{error}"))?;

    Ok(QueueSendOutput {
        message_id: msg.id,
        outbox_id: outbox.id,
    })
}

pub async fn active_whatsapp_account_id(
    db: &DatabaseConnection,
    tenant_id: i32,
) -> Result<i32, String> {
    tenant_whatsapp_account::Entity::find()
        .filter(tenant_whatsapp_account::Column::TenantId.eq(tenant_id))
        .filter(tenant_whatsapp_account::Column::IsActive.eq(true))
        .one(db)
        .await
        .map_err(|error| format!("DB query WhatsApp settings: {error}"))?
        .map(|account| account.id)
        .ok_or_else(|| "No active WhatsApp settings configured".to_string())
}

pub async fn ensure_contact_conversation(
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
        .filter(conversation::Column::ContactPhone.eq(phone))
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

#[allow(clippy::too_many_arguments)]
pub async fn create_queued_message(
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
    .map_err(|error| format!("DB insert message: {error}"))?;

    if let Ok(Some(conv)) = conversation::Entity::find_by_id(conversation_id)
        .one(db)
        .await
    {
        let mut update: conversation::ActiveModel = conv.into();
        update.last_message_at = Set(Some(now));
        let _ = update.update(db).await;
    }

    Ok(msg)
}

pub async fn create_outbox(
    db: &DatabaseConnection,
    tenant_id: i32,
    message_id: i32,
    kind: &str,
    payload: serde_json::Value,
) -> Result<outbox_message::Model, sea_orm::DbErr> {
    outbox_message::ActiveModel {
        tenant_id: Set(tenant_id),
        message_id: Set(message_id),
        kind: Set(kind.to_string()),
        payload_json: Set(payload),
        status: Set("pending".to_string()),
        attempts: Set(0),
        next_attempt_at: Set(chrono::Utc::now().naive_utc()),
        ..Default::default()
    }
    .insert(db)
    .await
}

pub async fn cleanup_message(db: &DatabaseConnection, message_id: i32) {
    let _ = message::Entity::delete_by_id(message_id).exec(db).await;
}
