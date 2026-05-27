use chrono::NaiveDateTime;
use sea_orm::Set;

use crate::domain::messaging::errors::{
    MessageDirection, MessageStatus, MessageType, MessagingError,
};
use crate::infrastructure::persistence::models::message as message_model;

/// A message in a conversation.
///
/// Invariants enforced:
/// - Outbound messages with success status (sent/delivered/read) MUST have wa_message_id set
/// - Status transitions follow valid state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    id: i32,
    conversation_id: i32,
    wa_message_id: Option<String>,
    direction: MessageDirection,
    msg_type: MessageType,
    body: Option<String>,
    media_url: Option<String>,
    media_mime: Option<String>,
    template_name: Option<String>,
    status: MessageStatus,
    timestamp: NaiveDateTime,
    tenant_id: Option<i32>,
    contact_id: Option<i32>,
    storage_key: Option<String>,
    original_filename: Option<String>,
    size_bytes: Option<i64>,
    created_at: NaiveDateTime,
}

impl Message {
    /// Create a new outbound text message for queuing
    pub fn new_outbound_text(
        conversation_id: i32,
        tenant_id: i32,
        contact_id: i32,
        body: String,
        timestamp: NaiveDateTime,
    ) -> Self {
        Self {
            id: 0,
            conversation_id,
            wa_message_id: None,
            direction: MessageDirection::Outbound,
            msg_type: MessageType::Text,
            body: Some(body),
            media_url: None,
            media_mime: None,
            template_name: None,
            status: MessageStatus::Queued,
            timestamp,
            tenant_id: Some(tenant_id),
            contact_id: Some(contact_id),
            storage_key: None,
            original_filename: None,
            size_bytes: None,
            created_at: timestamp,
        }
    }

    /// Create a new outbound template message for queuing
    pub fn new_outbound_template(
        conversation_id: i32,
        tenant_id: i32,
        contact_id: i32,
        template_name: String,
        timestamp: NaiveDateTime,
    ) -> Self {
        Self {
            id: 0,
            conversation_id,
            wa_message_id: None,
            direction: MessageDirection::Outbound,
            msg_type: MessageType::Template,
            body: None,
            media_url: None,
            media_mime: None,
            template_name: Some(template_name),
            status: MessageStatus::Queued,
            timestamp,
            tenant_id: Some(tenant_id),
            contact_id: Some(contact_id),
            storage_key: None,
            original_filename: None,
            size_bytes: None,
            created_at: timestamp,
        }
    }

    /// Create a new outbound media message for queuing
    #[allow(clippy::too_many_arguments)]
    pub fn new_outbound_media(
        conversation_id: i32,
        tenant_id: i32,
        contact_id: i32,
        msg_type: MessageType,
        caption: Option<String>,
        media_url: Option<String>,
        storage_key: Option<String>,
        original_filename: Option<String>,
        size_bytes: Option<i64>,
        timestamp: NaiveDateTime,
    ) -> Result<Self, MessagingError> {
        if !matches!(
            msg_type,
            MessageType::Image | MessageType::Document | MessageType::Audio | MessageType::Video
        ) {
            return Err(MessagingError::InvalidMessageType(format!(
                "{} is not a media type",
                msg_type.as_str()
            )));
        }

        Ok(Self {
            id: 0,
            conversation_id,
            wa_message_id: None,
            direction: MessageDirection::Outbound,
            msg_type,
            body: caption,
            media_url,
            media_mime: None,
            template_name: None,
            status: MessageStatus::Queued,
            timestamp,
            tenant_id: Some(tenant_id),
            contact_id: Some(contact_id),
            storage_key,
            original_filename,
            size_bytes,
            created_at: timestamp,
        })
    }

    /// Create a new inbound message
    pub fn new_inbound(
        conversation_id: i32,
        tenant_id: i32,
        contact_id: i32,
        wa_message_id: String,
        msg_type: MessageType,
        body: Option<String>,
        timestamp: NaiveDateTime,
    ) -> Self {
        Self {
            id: 0,
            conversation_id,
            wa_message_id: Some(wa_message_id),
            direction: MessageDirection::Inbound,
            msg_type,
            body,
            media_url: None,
            media_mime: None,
            template_name: None,
            status: MessageStatus::Received,
            timestamp,
            tenant_id: Some(tenant_id),
            contact_id: Some(contact_id),
            storage_key: None,
            original_filename: None,
            size_bytes: None,
            created_at: timestamp,
        }
    }

    /// Mark message as sent with WhatsApp message ID.
    /// This is the key fix for the known bug: ensures status=failed cannot have wa_message_id set
    /// through domain types - once we have a wa_message_id, status must be a success status.
    pub fn mark_sent(mut self, wa_message_id: String) -> Result<Self, MessagingError> {
        if wa_message_id.trim().is_empty() {
            return Err(MessagingError::MissingField("wa_message_id".into()));
        }
        self.wa_message_id = Some(wa_message_id);
        self.status = MessageStatus::Sent;
        Ok(self)
    }

    /// Mark message as failed.
    /// This enforces the invariant: cannot have wa_message_id with failed status.
    pub fn mark_failed(mut self) -> Result<Self, MessagingError> {
        // If we already have a wa_message_id, the message was sent successfully
        // at some point, so we should not allow marking it as failed.
        if self.wa_message_id.is_some() {
            return Err(MessagingError::InvalidState(
                "cannot mark message as failed when wa_message_id is already set".into(),
            ));
        }
        self.status = MessageStatus::Failed;
        Ok(self)
    }

    /// Update status from WhatsApp webhook status update.
    /// Allows transitions to delivered/read only if message was already sent.
    pub fn update_status(mut self, new_status: MessageStatus) -> Result<Self, MessagingError> {
        match new_status {
            MessageStatus::Delivered | MessageStatus::Read => {
                // These statuses require the message to have been sent first
                if self.wa_message_id.is_none() {
                    return Err(MessagingError::InvalidState(format!(
                        "cannot mark as {} without wa_message_id",
                        new_status.as_str()
                    )));
                }
                self.status = new_status;
                Ok(self)
            }
            MessageStatus::Sent => {
                // Should use mark_sent instead
                Err(MessagingError::InvalidState(
                    "use mark_sent to set sent status with wa_message_id".into(),
                ))
            }
            MessageStatus::Failed => self.mark_failed(),
            MessageStatus::Received | MessageStatus::Queued | MessageStatus::Sending => {
                self.status = new_status;
                Ok(self)
            }
        }
    }

    /// Set media info after upload
    pub fn set_media_info(
        mut self,
        media_url: String,
        media_mime: String,
        storage_key: String,
        size_bytes: i64,
    ) -> Self {
        self.media_url = Some(media_url);
        self.media_mime = Some(media_mime);
        self.storage_key = Some(storage_key);
        self.size_bytes = Some(size_bytes);
        self
    }

    // Getters
    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn conversation_id(&self) -> i32 {
        self.conversation_id
    }

    pub fn wa_message_id(&self) -> Option<&str> {
        self.wa_message_id.as_deref()
    }

    pub fn direction(&self) -> MessageDirection {
        self.direction
    }

    pub fn msg_type(&self) -> &MessageType {
        &self.msg_type
    }

    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub fn media_url(&self) -> Option<&str> {
        self.media_url.as_deref()
    }

    pub fn media_mime(&self) -> Option<&str> {
        self.media_mime.as_deref()
    }

    pub fn template_name(&self) -> Option<&str> {
        self.template_name.as_deref()
    }

    pub fn status(&self) -> MessageStatus {
        self.status
    }

    pub fn timestamp(&self) -> NaiveDateTime {
        self.timestamp
    }

    pub fn tenant_id(&self) -> Option<i32> {
        self.tenant_id
    }

    pub fn contact_id(&self) -> Option<i32> {
        self.contact_id
    }

    pub fn storage_key(&self) -> Option<&str> {
        self.storage_key.as_deref()
    }

    pub fn original_filename(&self) -> Option<&str> {
        self.original_filename.as_deref()
    }

    pub fn size_bytes(&self) -> Option<i64> {
        self.size_bytes
    }

    pub fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    /// Reconstruct from database model with validation
    #[allow(dead_code)]
    pub(crate) fn from_model(model: message_model::Model) -> Result<Self, MessagingError> {
        let direction = MessageDirection::parse(&model.direction)?;
        let msg_type = MessageType::parse(&model.msg_type)?;
        let status = MessageStatus::parse(&model.status)?;

        // Enforce invariant: success status must have wa_message_id
        if status.requires_wa_message_id() && model.wa_message_id.is_none() {
            return Err(MessagingError::InvalidState(format!(
                "message has status {} but no wa_message_id",
                status.as_str()
            )));
        }

        // Enforce invariant: failed status should not have wa_message_id
        if status == MessageStatus::Failed && model.wa_message_id.is_some() {
            return Err(MessagingError::InvalidState(
                "message has failed status but wa_message_id is set - data inconsistency".into(),
            ));
        }

        Ok(Self {
            id: model.id,
            conversation_id: model.conversation_id,
            wa_message_id: model.wa_message_id,
            direction,
            msg_type,
            body: model.body,
            media_url: model.media_url,
            media_mime: model.media_mime,
            template_name: model.template_name,
            status,
            timestamp: model.timestamp,
            tenant_id: model.tenant_id,
            contact_id: model.contact_id,
            storage_key: model.storage_key,
            original_filename: model.original_filename,
            size_bytes: model.size_bytes,
            created_at: model.created_at,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn to_active_model(&self) -> message_model::ActiveModel {
        use sea_orm::ActiveValue::NotSet;

        message_model::ActiveModel {
            id: if self.id == 0 { NotSet } else { Set(self.id) },
            conversation_id: Set(self.conversation_id),
            wa_message_id: Set(self.wa_message_id.clone()),
            direction: Set(self.direction.as_str().to_string()),
            msg_type: Set(self.msg_type.as_str().to_string()),
            body: Set(self.body.clone()),
            media_url: Set(self.media_url.clone()),
            media_mime: Set(self.media_mime.clone()),
            template_name: Set(self.template_name.clone()),
            status: Set(self.status.as_str().to_string()),
            timestamp: Set(self.timestamp),
            tenant_id: Set(self.tenant_id),
            contact_id: Set(self.contact_id),
            storage_key: Set(self.storage_key.clone()),
            original_filename: Set(self.original_filename.clone()),
            size_bytes: Set(self.size_bytes),
            created_at: if self.id == 0 {
                NotSet
            } else {
                Set(self.created_at)
            },
        }
    }

    #[allow(dead_code)]
    pub(crate) fn set_id(mut self, id: i32) -> Self {
        self.id = id;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_timestamp() -> NaiveDateTime {
        Utc::now().naive_utc()
    }

    #[test]
    fn new_outbound_text_starts_queued() {
        let msg = Message::new_outbound_text(1, 1, 1, "Hello".into(), test_timestamp());
        assert_eq!(msg.status(), MessageStatus::Queued);
        assert_eq!(msg.direction(), MessageDirection::Outbound);
        assert_eq!(msg.msg_type(), &MessageType::Text);
        assert!(msg.wa_message_id().is_none());
    }

    #[test]
    fn mark_sent_sets_wa_message_id_and_status() {
        let msg = Message::new_outbound_text(1, 1, 1, "Hello".into(), test_timestamp());
        let sent = msg.mark_sent("wamid.123".into()).unwrap();

        assert_eq!(sent.wa_message_id(), Some("wamid.123"));
        assert_eq!(sent.status(), MessageStatus::Sent);
    }

    #[test]
    fn mark_failed_rejects_when_wa_message_id_set() {
        let msg = Message::new_outbound_text(1, 1, 1, "Hello".into(), test_timestamp());
        let sent = msg.mark_sent("wamid.123".into()).unwrap();

        // This is the key bug fix: cannot mark as failed after successful send
        let result = sent.mark_failed();
        assert!(matches!(result, Err(MessagingError::InvalidState(_))));
    }

    #[test]
    fn mark_failed_allowed_when_no_wa_message_id() {
        let msg = Message::new_outbound_text(1, 1, 1, "Hello".into(), test_timestamp());
        let failed = msg.mark_failed().unwrap();

        assert_eq!(failed.status(), MessageStatus::Failed);
        assert!(failed.wa_message_id().is_none());
    }

    #[test]
    fn from_model_rejects_failed_with_wa_message_id() {
        // This tests the bug: outbound message with wa_message_id but status=failed
        let model = message_model::Model {
            id: 1,
            conversation_id: 1,
            wa_message_id: Some("wamid.123".into()),
            direction: "outbound".into(),
            msg_type: "text".into(),
            body: Some("Hello".into()),
            media_url: None,
            media_mime: None,
            template_name: None,
            status: "failed".into(),
            timestamp: test_timestamp(),
            tenant_id: Some(1),
            contact_id: Some(1),
            storage_key: None,
            original_filename: None,
            size_bytes: None,
            created_at: test_timestamp(),
        };

        let result = Message::from_model(model);
        assert!(matches!(result, Err(MessagingError::InvalidState(_))));
    }

    #[test]
    fn from_model_rejects_sent_without_wa_message_id() {
        let model = message_model::Model {
            id: 1,
            conversation_id: 1,
            wa_message_id: None,
            direction: "outbound".into(),
            msg_type: "text".into(),
            body: Some("Hello".into()),
            media_url: None,
            media_mime: None,
            template_name: None,
            status: "sent".into(),
            timestamp: test_timestamp(),
            tenant_id: Some(1),
            contact_id: Some(1),
            storage_key: None,
            original_filename: None,
            size_bytes: None,
            created_at: test_timestamp(),
        };

        let result = Message::from_model(model);
        assert!(matches!(result, Err(MessagingError::InvalidState(_))));
    }

    #[test]
    fn from_model_accepts_sent_with_wa_message_id() {
        let model = message_model::Model {
            id: 1,
            conversation_id: 1,
            wa_message_id: Some("wamid.123".into()),
            direction: "outbound".into(),
            msg_type: "text".into(),
            body: Some("Hello".into()),
            media_url: None,
            media_mime: None,
            template_name: None,
            status: "sent".into(),
            timestamp: test_timestamp(),
            tenant_id: Some(1),
            contact_id: Some(1),
            storage_key: None,
            original_filename: None,
            size_bytes: None,
            created_at: test_timestamp(),
        };

        let msg = Message::from_model(model).unwrap();
        assert_eq!(msg.status(), MessageStatus::Sent);
        assert_eq!(msg.wa_message_id(), Some("wamid.123"));
    }

    #[test]
    fn update_status_to_delivered_requires_wa_message_id() {
        let msg = Message::new_outbound_text(1, 1, 1, "Hello".into(), test_timestamp());

        let result = msg.update_status(MessageStatus::Delivered);
        assert!(matches!(result, Err(MessagingError::InvalidState(_))));
    }

    #[test]
    fn update_status_to_delivered_works_after_sent() {
        let msg = Message::new_outbound_text(1, 1, 1, "Hello".into(), test_timestamp());
        let sent = msg.mark_sent("wamid.123".into()).unwrap();

        let delivered = sent.update_status(MessageStatus::Delivered).unwrap();
        assert_eq!(delivered.status(), MessageStatus::Delivered);
    }

    #[test]
    fn new_outbound_media_rejects_text_type() {
        let result = Message::new_outbound_media(
            1,
            1,
            1,
            MessageType::Text, // Invalid for media
            None,
            None,
            None,
            None,
            None,
            test_timestamp(),
        );

        assert!(matches!(result, Err(MessagingError::InvalidMessageType(_))));
    }

    #[test]
    fn new_outbound_media_accepts_image_type() {
        let msg = Message::new_outbound_media(
            1,
            1,
            1,
            MessageType::Image,
            Some("Check this".into()),
            Some("https://example.com/img.jpg".into()),
            None,
            None,
            None,
            test_timestamp(),
        )
        .unwrap();

        assert_eq!(msg.msg_type(), &MessageType::Image);
        assert_eq!(msg.body(), Some("Check this"));
    }
}
