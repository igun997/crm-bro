use async_trait::async_trait;

use crate::domain::messaging::{Message, MessagingError};

#[async_trait]
pub trait MessageRepository: Send + Sync {
    /// Find a message by ID within a tenant
    async fn find_by_id(&self, id: i32, tenant_id: i32) -> Result<Option<Message>, MessagingError>;

    /// Find a message by WhatsApp message ID
    async fn find_by_wa_message_id(
        &self,
        wa_message_id: &str,
    ) -> Result<Option<Message>, MessagingError>;

    /// Save a new message (insert or update)
    async fn save(&self, message: &Message) -> Result<Message, MessagingError>;

    /// Update message status (used by webhook status updates)
    async fn update_status(&self, id: i32, status: &str) -> Result<(), MessagingError>;
}
