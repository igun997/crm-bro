use crate::domain::messaging::{MessageRepository, MessagingError, Outbox, OutboxRepository};

const MAX_ATTEMPTS: i32 = 5;

/// Service for processing outbox jobs with proper state management.
///
/// This service handles the claim/send/update workflow and ensures that
/// message status invariants are maintained:
/// - If send succeeds, message gets wa_message_id and status=sent
/// - If send fails, message gets status=failed only if no wa_message_id exists
pub struct OutboxService<MR: MessageRepository, OR: OutboxRepository> {
    message_repo: MR,
    outbox_repo: OR,
}

impl<MR: MessageRepository, OR: OutboxRepository> OutboxService<MR, OR> {
    pub fn new(message_repo: MR, outbox_repo: OR) -> Self {
        Self {
            message_repo,
            outbox_repo,
        }
    }

    /// Fetch ready jobs for processing
    pub async fn fetch_ready_jobs(&self, limit: u64) -> Result<Vec<Outbox>, MessagingError> {
        self.outbox_repo.find_ready_jobs(limit).await
    }

    /// Claim a job for processing.
    /// Returns the claimed job, or None if already claimed by another worker.
    pub async fn claim_job(&self, job: &Outbox) -> Result<Option<Outbox>, MessagingError> {
        self.outbox_repo.claim_job(job).await
    }

    /// Mark a job as successfully completed.
    /// Updates both the outbox entry and the associated message.
    /// The message gets wa_message_id set and status=sent.
    pub async fn mark_success(
        &self,
        job: Outbox,
        wa_message_id: String,
    ) -> Result<(), MessagingError> {
        let message = self
            .message_repo
            .find_by_id(job.message_id(), job.tenant_id())
            .await?
            .ok_or(MessagingError::MessageNotFound(job.message_id()))?;

        // Domain type enforces invariant: setting wa_message_id also sets status=sent
        let sent_message = message.mark_sent(wa_message_id)?;
        self.message_repo.save(&sent_message).await?;

        let done_job = job.mark_done();
        self.outbox_repo.save(&done_job).await?;

        Ok(())
    }

    /// Mark a job as failed.
    /// Updates both the outbox entry and the associated message.
    /// Message status becomes "failed" only if no wa_message_id exists.
    pub async fn mark_failure(&self, job: Outbox, error: String) -> Result<(), MessagingError> {
        let message = self
            .message_repo
            .find_by_id(job.message_id(), job.tenant_id())
            .await?
            .ok_or(MessagingError::MessageNotFound(job.message_id()))?;

        // Domain type enforces invariant: cannot mark failed if wa_message_id is set
        // If the message was somehow sent successfully before, we should not mark it failed
        let failed_message = match message.mark_failed() {
            Ok(msg) => msg,
            Err(MessagingError::InvalidState(_)) => {
                // Message already has wa_message_id, log warning but don't fail
                tracing::warn!(
                    message_id = job.message_id(),
                    "Attempted to mark message as failed but wa_message_id is set"
                );
                // Don't update message status, just mark outbox as failed
                let failed_job = job.mark_failed(error, MAX_ATTEMPTS);
                self.outbox_repo.save(&failed_job).await?;
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        self.message_repo.save(&failed_message).await?;

        let failed_job = job.mark_failed(error, MAX_ATTEMPTS);
        self.outbox_repo.save(&failed_job).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::Utc;
    use mockall::{mock, predicate::*};

    use crate::domain::messaging::Message;

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

    fn make_queued_message(id: i32, tenant_id: i32) -> Message {
        Message::new_outbound_text(1, tenant_id, 1, "Hello".into(), Utc::now().naive_utc())
            .set_id(id)
    }

    fn make_pending_outbox(id: i32, tenant_id: i32, message_id: i32) -> Outbox {
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
    async fn mark_success_sets_wa_message_id_and_sent_status() {
        let mut msg_repo = MockMessageRepo::new();
        let mut outbox_repo = MockOutboxRepo::new();

        let msg = make_queued_message(42, 1);
        let outbox = make_pending_outbox(7, 1, 42);

        msg_repo
            .expect_find_by_id()
            .with(eq(42), eq(1))
            .times(1)
            .returning(move |_, _| Ok(Some(msg.clone())));
        msg_repo.expect_save().times(1).returning(|m| Ok(m.clone()));
        outbox_repo
            .expect_save()
            .times(1)
            .returning(|o| Ok(o.clone()));

        let service = OutboxService::new(msg_repo, outbox_repo);
        service
            .mark_success(outbox, "wamid.123".into())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn mark_failure_updates_status() {
        let mut msg_repo = MockMessageRepo::new();
        let mut outbox_repo = MockOutboxRepo::new();

        let msg = make_queued_message(42, 1);
        let outbox = make_pending_outbox(7, 1, 42);

        msg_repo
            .expect_find_by_id()
            .with(eq(42), eq(1))
            .times(1)
            .returning(move |_, _| Ok(Some(msg.clone())));
        msg_repo.expect_save().times(1).returning(|m| Ok(m.clone()));
        outbox_repo
            .expect_save()
            .times(1)
            .returning(|o| Ok(o.clone()));

        let service = OutboxService::new(msg_repo, outbox_repo);
        service
            .mark_failure(outbox, "Network error".into())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn mark_failure_does_not_update_message_if_wa_message_id_set() {
        let mut msg_repo = MockMessageRepo::new();
        let mut outbox_repo = MockOutboxRepo::new();

        // Message was already sent
        let sent_msg = make_queued_message(42, 1)
            .mark_sent("wamid.123".into())
            .unwrap();
        let outbox = make_pending_outbox(7, 1, 42);

        msg_repo
            .expect_find_by_id()
            .with(eq(42), eq(1))
            .times(1)
            .returning(move |_, _| Ok(Some(sent_msg.clone())));
        // Message save should NOT be called since we shouldn't update sent message to failed
        msg_repo.expect_save().times(0);
        outbox_repo
            .expect_save()
            .times(1)
            .returning(|o| Ok(o.clone()));

        let service = OutboxService::new(msg_repo, outbox_repo);
        service
            .mark_failure(outbox, "Network error".into())
            .await
            .unwrap();
    }
}
