use chrono::Utc;
use serde_json::Value;

use crate::domain::messaging::{
    Message, MessageRepository, MessageType, MessagingError, Outbox, OutboxRepository,
};

/// Service for creating outbound messages and managing the outbox queue.
///
/// This service enforces the key invariant: a message with wa_message_id set
/// cannot have status=failed. The domain types enforce this, and this service
/// coordinates the message+outbox creation workflow.
pub struct ChatService<MR: MessageRepository, OR: OutboxRepository> {
    message_repo: MR,
    outbox_repo: OR,
}

impl<MR: MessageRepository, OR: OutboxRepository> ChatService<MR, OR> {
    pub fn new(message_repo: MR, outbox_repo: OR) -> Self {
        Self {
            message_repo,
            outbox_repo,
        }
    }

    /// Queue a text message for sending
    pub async fn queue_text(
        &self,
        tenant_id: i32,
        contact_id: i32,
        conversation_id: i32,
        to: String,
        message: String,
    ) -> Result<(Message, Outbox), MessagingError> {
        let now = Utc::now().naive_utc();

        let msg = Message::new_outbound_text(
            conversation_id,
            tenant_id,
            contact_id,
            message.clone(),
            now,
        );
        let msg = self.message_repo.save(&msg).await?;

        let payload = serde_json::json!({
            "type": "text",
            "to": to,
            "message": message,
        });
        let outbox = self
            .create_outbox(tenant_id, msg.id(), "send_text", payload)
            .await?;

        Ok((msg, outbox))
    }

    /// Queue a template message for sending
    pub async fn queue_template(
        &self,
        tenant_id: i32,
        contact_id: i32,
        conversation_id: i32,
        to: String,
        template_name: String,
        language: String,
    ) -> Result<(Message, Outbox), MessagingError> {
        let now = Utc::now().naive_utc();

        let msg = Message::new_outbound_template(
            conversation_id,
            tenant_id,
            contact_id,
            template_name.clone(),
            now,
        );
        let msg = self.message_repo.save(&msg).await?;

        let payload = serde_json::json!({
            "type": "template",
            "to": to,
            "template_name": template_name,
            "language": language,
        });
        let outbox = self
            .create_outbox(tenant_id, msg.id(), "send_template", payload)
            .await?;

        Ok((msg, outbox))
    }

    /// Queue a media message for sending
    #[allow(clippy::too_many_arguments)]
    pub async fn queue_media(
        &self,
        tenant_id: i32,
        contact_id: i32,
        conversation_id: i32,
        to: String,
        media_type: MessageType,
        url: String,
        caption: Option<String>,
    ) -> Result<(Message, Outbox), MessagingError> {
        let now = Utc::now().naive_utc();

        let msg = Message::new_outbound_media(
            conversation_id,
            tenant_id,
            contact_id,
            media_type.clone(),
            caption.clone(),
            Some(url.clone()),
            None,
            None,
            None,
            now,
        )?;
        let msg = self.message_repo.save(&msg).await?;

        let payload = serde_json::json!({
            "type": "media",
            "to": to,
            "media_type": media_type.as_str(),
            "url": url,
            "caption": caption,
        });
        let outbox = self
            .create_outbox(tenant_id, msg.id(), "send_media", payload)
            .await?;

        Ok((msg, outbox))
    }

    /// Mark a message as sent with its WhatsApp message ID.
    /// This is called after successful send by the outbox worker.
    /// The domain types ensure we cannot have wa_message_id with failed status.
    pub async fn mark_message_sent(
        &self,
        message_id: i32,
        tenant_id: i32,
        wa_message_id: String,
    ) -> Result<Message, MessagingError> {
        let msg = self
            .message_repo
            .find_by_id(message_id, tenant_id)
            .await?
            .ok_or(MessagingError::MessageNotFound(message_id))?;

        // Domain type enforces: if wa_message_id is set, status must be "sent"
        let sent_msg = msg.mark_sent(wa_message_id)?;
        self.message_repo.save(&sent_msg).await
    }

    /// Mark a message as failed.
    /// Domain types prevent this if wa_message_id is already set.
    pub async fn mark_message_failed(
        &self,
        message_id: i32,
        tenant_id: i32,
    ) -> Result<Message, MessagingError> {
        let msg = self
            .message_repo
            .find_by_id(message_id, tenant_id)
            .await?
            .ok_or(MessagingError::MessageNotFound(message_id))?;

        // Domain type enforces: cannot mark failed if wa_message_id is set
        let failed_msg = msg.mark_failed()?;
        self.message_repo.save(&failed_msg).await
    }

    async fn create_outbox(
        &self,
        tenant_id: i32,
        message_id: i32,
        kind: &str,
        payload: Value,
    ) -> Result<Outbox, MessagingError> {
        let outbox = Outbox::new(tenant_id, message_id, kind.to_string(), payload)?;
        self.outbox_repo.save(&outbox).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mockall::{mock, predicate::*};

    mock! {
        MessageRepo {}

        #[async_trait]
        impl MessageRepository for MessageRepo {
            async fn find_by_id(&self, id: i32, tenant_id: i32) -> Result<Option<Message>, MessagingError>;
            async fn find_by_wa_message_id(&self, wa_message_id: &str) -> Result<Option<Message>, MessagingError>;
            async fn save(&self, message: &Message) -> Result<Message, MessagingError>;
            async fn update_status(&self, id: i32, status: &str) -> Result<(), MessagingError>;
        }
    }

    mock! {
        OutboxRepo {}

        #[async_trait]
        impl OutboxRepository for OutboxRepo {
            async fn find_by_id(&self, id: i32) -> Result<Option<Outbox>, MessagingError>;
            async fn find_ready_jobs(&self, limit: u64) -> Result<Vec<Outbox>, MessagingError>;
            async fn claim_job(&self, job: &Outbox) -> Result<Option<Outbox>, MessagingError>;
            async fn save(&self, outbox: &Outbox) -> Result<Outbox, MessagingError>;
        }
    }

    fn make_saved_message(id: i32, tenant_id: i32, conversation_id: i32) -> Message {
        Message::new_outbound_text(
            conversation_id,
            tenant_id,
            1,
            "Hello".into(),
            Utc::now().naive_utc(),
        )
        .set_id(id)
    }

    fn make_saved_outbox(id: i32, tenant_id: i32, message_id: i32) -> Outbox {
        Outbox::new(
            tenant_id,
            message_id,
            "send_text".into(),
            serde_json::json!({"type":"text"}),
        )
        .unwrap()
        .set_id(id)
    }

    #[tokio::test]
    async fn queue_text_creates_message_and_outbox() {
        let mut msg_repo = MockMessageRepo::new();
        let mut outbox_repo = MockOutboxRepo::new();

        let _saved_msg = make_saved_message(42, 1, 1);
        let saved_outbox = make_saved_outbox(7, 1, 42);

        msg_repo
            .expect_save()
            .times(1)
            .returning(move |msg| Ok(msg.clone().set_id(42)));
        outbox_repo
            .expect_save()
            .times(1)
            .returning(move |_| Ok(saved_outbox.clone()));

        let service = ChatService::new(msg_repo, outbox_repo);
        let (msg, outbox) = service
            .queue_text(1, 1, 1, "628996926184".into(), "Hello".into())
            .await
            .unwrap();

        assert_eq!(msg.id(), 42);
        assert_eq!(outbox.id(), 7);
    }

    #[tokio::test]
    async fn mark_message_sent_updates_status() {
        let mut msg_repo = MockMessageRepo::new();

        let saved_msg = make_saved_message(42, 1, 1);
        msg_repo
            .expect_find_by_id()
            .with(eq(42), eq(1))
            .times(1)
            .returning(move |_, _| Ok(Some(saved_msg.clone())));
        msg_repo
            .expect_save()
            .times(1)
            .returning(|msg| Ok(msg.clone()));

        let outbox_repo = MockOutboxRepo::new();
        let service = ChatService::new(msg_repo, outbox_repo);

        let result = service.mark_message_sent(42, 1, "wamid.123".into()).await;

        assert!(result.is_ok());
        let msg = result.unwrap();
        assert_eq!(msg.wa_message_id(), Some("wamid.123"));
    }

    #[tokio::test]
    async fn mark_message_failed_rejects_when_wa_message_id_set() {
        let mut msg_repo = MockMessageRepo::new();

        // Create a message that was already sent
        let sent_msg = make_saved_message(42, 1, 1)
            .mark_sent("wamid.123".into())
            .unwrap();

        msg_repo
            .expect_find_by_id()
            .with(eq(42), eq(1))
            .times(1)
            .returning(move |_, _| Ok(Some(sent_msg.clone())));
        // save should NOT be called since mark_failed should fail
        msg_repo.expect_save().times(0);

        let outbox_repo = MockOutboxRepo::new();
        let service = ChatService::new(msg_repo, outbox_repo);

        let result = service.mark_message_failed(42, 1).await;

        assert!(matches!(result, Err(MessagingError::InvalidState(_))));
    }
}
