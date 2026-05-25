use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MessagingError {
    #[error("Message not found: {0}")]
    MessageNotFound(i32),
    #[error("Conversation not found: {0}")]
    ConversationNotFound(i32),
    #[error("Outbox not found: {0}")]
    OutboxNotFound(i32),
    #[error("Invalid message direction: {0}")]
    InvalidDirection(String),
    #[error("Invalid message type: {0}")]
    InvalidMessageType(String),
    #[error("Invalid message status: {0}")]
    InvalidStatus(String),
    #[error("Invalid message state: {0}")]
    InvalidState(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Storage error: {0}")]
    Storage(String),
}

/// Valid message directions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageDirection {
    Inbound,
    Outbound,
}

impl MessageDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, MessagingError> {
        match s {
            "inbound" => Ok(Self::Inbound),
            "outbound" => Ok(Self::Outbound),
            other => Err(MessagingError::InvalidDirection(other.to_string())),
        }
    }
}

/// Valid message types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageType {
    Text,
    Image,
    Document,
    Audio,
    Video,
    Template,
}

impl MessageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Document => "document",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Template => "template",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, MessagingError> {
        match s {
            "text" => Ok(Self::Text),
            "image" => Ok(Self::Image),
            "document" => Ok(Self::Document),
            "audio" => Ok(Self::Audio),
            "video" => Ok(Self::Video),
            "template" => Ok(Self::Template),
            other => Err(MessagingError::InvalidMessageType(other.to_string())),
        }
    }
}

/// Valid message statuses with state invariants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus {
    /// Message received from contact
    Received,
    /// Message queued for sending
    Queued,
    /// Message currently being sent
    Sending,
    /// Message successfully sent to WhatsApp
    Sent,
    /// Message delivered to contact
    Delivered,
    /// Message read by contact
    Read,
    /// Message failed to send after max retries
    Failed,
}

impl MessageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Delivered => "delivered",
            Self::Read => "read",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, MessagingError> {
        match s {
            "received" => Ok(Self::Received),
            "queued" => Ok(Self::Queued),
            "sending" => Ok(Self::Sending),
            "sent" => Ok(Self::Sent),
            "delivered" => Ok(Self::Delivered),
            "read" => Ok(Self::Read),
            "failed" => Ok(Self::Failed),
            other => Err(MessagingError::InvalidStatus(other.to_string())),
        }
    }

    /// Returns true if this status indicates the message was successfully sent
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Sent | Self::Delivered | Self::Read)
    }

    /// Returns true if wa_message_id should be present for this status
    pub fn requires_wa_message_id(&self) -> bool {
        matches!(self, Self::Sent | Self::Delivered | Self::Read)
    }
}

/// Valid outbox statuses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxStatus {
    Pending,
    Processing,
    Done,
    Failed,
}

impl OutboxStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, MessagingError> {
        match s {
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "done" => Ok(Self::Done),
            "failed" => Ok(Self::Failed),
            other => Err(MessagingError::InvalidStatus(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_status_sent_is_success() {
        assert!(MessageStatus::Sent.is_success());
        assert!(MessageStatus::Delivered.is_success());
        assert!(MessageStatus::Read.is_success());
        assert!(!MessageStatus::Failed.is_success());
        assert!(!MessageStatus::Queued.is_success());
    }

    #[test]
    fn message_status_sent_requires_wa_message_id() {
        assert!(MessageStatus::Sent.requires_wa_message_id());
        assert!(MessageStatus::Delivered.requires_wa_message_id());
        assert!(!MessageStatus::Failed.requires_wa_message_id());
        assert!(!MessageStatus::Queued.requires_wa_message_id());
    }

    #[test]
    fn message_status_roundtrip() {
        for status in [
            MessageStatus::Received,
            MessageStatus::Queued,
            MessageStatus::Sending,
            MessageStatus::Sent,
            MessageStatus::Delivered,
            MessageStatus::Read,
            MessageStatus::Failed,
        ] {
            assert_eq!(
                MessageStatus::from_str(status.as_str()),
                Ok(status),
                "Status roundtrip failed for {:?}",
                status
            );
        }
    }
}
