use chrono::NaiveDateTime;
use sea_orm::Set;
use serde_json::Value;

use crate::domain::messaging::errors::{MessagingError, OutboxStatus};
use crate::models::outbox_message as outbox_model;

/// Outbox entry for a message to be sent via WhatsApp.
#[derive(Debug, Clone)]
pub struct Outbox {
    id: i32,
    tenant_id: i32,
    message_id: i32,
    kind: String,
    payload_json: Value,
    status: OutboxStatus,
    attempts: i32,
    last_error: Option<String>,
    next_attempt_at: NaiveDateTime,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

impl Outbox {
    /// Create a new outbox entry for a message
    pub fn new(
        tenant_id: i32,
        message_id: i32,
        kind: String,
        payload_json: Value,
    ) -> Result<Self, MessagingError> {
        let kind = kind.trim();
        if kind.is_empty() {
            return Err(MessagingError::MissingField("kind".into()));
        }

        let now = chrono::Utc::now().naive_utc();
        Ok(Self {
            id: 0,
            tenant_id,
            message_id,
            kind: kind.to_string(),
            payload_json,
            status: OutboxStatus::Pending,
            attempts: 0,
            last_error: None,
            next_attempt_at: now,
            created_at: now,
            updated_at: now,
        })
    }

    /// Mark as processing (claimed by worker)
    pub fn mark_processing(mut self) -> Self {
        self.status = OutboxStatus::Processing;
        self.updated_at = chrono::Utc::now().naive_utc();
        self
    }

    /// Mark as done after successful send
    pub fn mark_done(mut self) -> Self {
        self.status = OutboxStatus::Done;
        self.last_error = None;
        self.updated_at = chrono::Utc::now().naive_utc();
        self
    }

    /// Mark as failed with error and schedule retry
    pub fn mark_failed(mut self, error: String, max_attempts: i32) -> Self {
        self.attempts += 1;
        self.last_error = Some(error);
        self.updated_at = chrono::Utc::now().naive_utc();

        if self.attempts >= max_attempts {
            self.status = OutboxStatus::Failed;
        } else {
            self.status = OutboxStatus::Pending;
            // Exponential backoff
            let delay_secs = retry_delay_seconds(self.attempts);
            self.next_attempt_at =
                chrono::Utc::now().naive_utc() + chrono::Duration::seconds(delay_secs);
        }
        self
    }

    /// Check if this job is ready to be processed
    pub fn is_ready(&self) -> bool {
        match self.status {
            OutboxStatus::Pending => self.next_attempt_at <= chrono::Utc::now().naive_utc(),
            OutboxStatus::Processing => {
                // Stale processing (older than 10 minutes)
                let stale_cutoff = chrono::Utc::now().naive_utc() - chrono::Duration::minutes(10);
                self.updated_at <= stale_cutoff
            }
            OutboxStatus::Done | OutboxStatus::Failed => false,
        }
    }

    // Getters
    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn tenant_id(&self) -> i32 {
        self.tenant_id
    }

    pub fn message_id(&self) -> i32 {
        self.message_id
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn payload_json(&self) -> &Value {
        &self.payload_json
    }

    pub fn status(&self) -> OutboxStatus {
        self.status
    }

    pub fn attempts(&self) -> i32 {
        self.attempts
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn next_attempt_at(&self) -> NaiveDateTime {
        self.next_attempt_at
    }

    pub fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    pub fn updated_at(&self) -> NaiveDateTime {
        self.updated_at
    }

    #[allow(dead_code)]
    pub(crate) fn from_model(model: outbox_model::Model) -> Result<Self, MessagingError> {
        let status = OutboxStatus::parse(&model.status)?;

        Ok(Self {
            id: model.id,
            tenant_id: model.tenant_id,
            message_id: model.message_id,
            kind: model.kind,
            payload_json: model.payload_json,
            status,
            attempts: model.attempts,
            last_error: model.last_error,
            next_attempt_at: model.next_attempt_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn to_active_model(&self) -> outbox_model::ActiveModel {
        use sea_orm::ActiveValue::NotSet;

        outbox_model::ActiveModel {
            id: if self.id == 0 { NotSet } else { Set(self.id) },
            tenant_id: Set(self.tenant_id),
            message_id: Set(self.message_id),
            kind: Set(self.kind.clone()),
            payload_json: Set(self.payload_json.clone()),
            status: Set(self.status.as_str().to_string()),
            attempts: Set(self.attempts),
            last_error: Set(self.last_error.clone()),
            next_attempt_at: Set(self.next_attempt_at),
            created_at: if self.id == 0 {
                NotSet
            } else {
                Set(self.created_at)
            },
            updated_at: Set(self.updated_at),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn set_id(mut self, id: i32) -> Self {
        self.id = id;
        self
    }
}

fn retry_delay_seconds(attempts: i32) -> i64 {
    match attempts {
        0 | 1 => 10,
        2 => 30,
        3 => 60,
        _ => 300,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_payload() -> Value {
        json!({
            "type": "text",
            "to": "628996926184",
            "message": "Hello"
        })
    }

    #[test]
    fn new_outbox_starts_pending() {
        let outbox = Outbox::new(1, 1, "send_text".into(), test_payload()).unwrap();

        assert_eq!(outbox.status(), OutboxStatus::Pending);
        assert_eq!(outbox.attempts(), 0);
        assert!(outbox.last_error().is_none());
    }

    #[test]
    fn mark_done_sets_status() {
        let outbox = Outbox::new(1, 1, "send_text".into(), test_payload()).unwrap();
        let done = outbox.mark_done();

        assert_eq!(done.status(), OutboxStatus::Done);
        assert!(done.last_error().is_none());
    }

    #[test]
    fn mark_failed_increments_attempts() {
        let outbox = Outbox::new(1, 1, "send_text".into(), test_payload()).unwrap();
        let failed = outbox.mark_failed("Network error".into(), 5);

        assert_eq!(failed.attempts(), 1);
        assert_eq!(failed.status(), OutboxStatus::Pending); // Not terminal yet
        assert_eq!(failed.last_error(), Some("Network error"));
    }

    #[test]
    fn mark_failed_becomes_terminal_after_max_attempts() {
        let outbox = Outbox::new(1, 1, "send_text".into(), test_payload()).unwrap();
        let result = outbox
            .mark_failed("Error 1".into(), 3)
            .mark_failed("Error 2".into(), 3)
            .mark_failed("Error 3".into(), 3);

        assert_eq!(result.attempts(), 3);
        assert_eq!(result.status(), OutboxStatus::Failed); // Terminal
    }

    #[test]
    fn is_ready_for_pending_with_past_next_attempt() {
        let mut outbox = Outbox::new(1, 1, "send_text".into(), test_payload()).unwrap();
        // Make next_attempt_at in the past
        outbox.next_attempt_at = chrono::Utc::now().naive_utc() - chrono::Duration::seconds(10);

        assert!(outbox.is_ready());
    }

    #[test]
    fn is_not_ready_for_done() {
        let outbox = Outbox::new(1, 1, "send_text".into(), test_payload()).unwrap();
        let done = outbox.mark_done();

        assert!(!done.is_ready());
    }
}
