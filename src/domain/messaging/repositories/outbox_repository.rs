use async_trait::async_trait;

use crate::domain::messaging::{MessagingError, Outbox};

#[async_trait]
pub trait OutboxRepository: Send + Sync {
    /// Find an outbox entry by ID
    async fn find_by_id(&self, id: i32) -> Result<Option<Outbox>, MessagingError>;

    /// Find ready jobs for processing
    /// Returns jobs that are either:
    /// - pending with next_attempt_at <= now
    /// - processing with updated_at older than stale cutoff (10 min)
    async fn find_ready_jobs(&self, limit: u64) -> Result<Vec<Outbox>, MessagingError>;

    /// Claim a job for processing (optimistic lock)
    /// Returns the claimed job, or None if already claimed
    async fn claim_job(&self, job: &Outbox) -> Result<Option<Outbox>, MessagingError>;

    /// Save an outbox entry (insert or update)
    async fn save(&self, outbox: &Outbox) -> Result<Outbox, MessagingError>;
}
