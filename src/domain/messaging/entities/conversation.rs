use chrono::NaiveDateTime;
use sea_orm::Set;

use crate::domain::messaging::errors::MessagingError;
use crate::infrastructure::persistence::models::conversation as conversation_model;

/// A conversation between a tenant and a contact via WhatsApp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    id: i32,
    contact_phone: String,
    contact_name: Option<String>,
    last_message_at: Option<NaiveDateTime>,
    tenant_id: Option<i32>,
    contact_id: Option<i32>,
    whatsapp_account_id: Option<i32>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

impl Conversation {
    /// Create a new conversation
    pub fn new(
        tenant_id: i32,
        contact_id: i32,
        contact_phone: String,
        contact_name: Option<String>,
        whatsapp_account_id: i32,
    ) -> Result<Self, MessagingError> {
        let phone = contact_phone.trim();
        if phone.is_empty() {
            return Err(MessagingError::MissingField("contact_phone".into()));
        }

        let now = chrono::Utc::now().naive_utc();
        Ok(Self {
            id: 0,
            contact_phone: phone.to_string(),
            contact_name,
            last_message_at: None,
            tenant_id: Some(tenant_id),
            contact_id: Some(contact_id),
            whatsapp_account_id: Some(whatsapp_account_id),
            created_at: now,
            updated_at: now,
        })
    }

    /// Update last message timestamp
    pub fn touch(mut self, timestamp: NaiveDateTime) -> Self {
        self.last_message_at = Some(timestamp);
        self.updated_at = timestamp;
        self
    }

    /// Update contact name (typically from inbound message profile)
    pub fn set_contact_name(mut self, name: Option<String>) -> Self {
        self.contact_name = name;
        self
    }

    // Getters
    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn contact_phone(&self) -> &str {
        &self.contact_phone
    }

    pub fn contact_name(&self) -> Option<&str> {
        self.contact_name.as_deref()
    }

    pub fn last_message_at(&self) -> Option<NaiveDateTime> {
        self.last_message_at
    }

    pub fn tenant_id(&self) -> Option<i32> {
        self.tenant_id
    }

    pub fn contact_id(&self) -> Option<i32> {
        self.contact_id
    }

    pub fn whatsapp_account_id(&self) -> Option<i32> {
        self.whatsapp_account_id
    }

    pub fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    pub fn updated_at(&self) -> NaiveDateTime {
        self.updated_at
    }

    #[allow(dead_code)]
    pub(crate) fn from_model(model: conversation_model::Model) -> Self {
        Self {
            id: model.id,
            contact_phone: model.contact_phone,
            contact_name: model.contact_name,
            last_message_at: model.last_message_at,
            tenant_id: model.tenant_id,
            contact_id: model.contact_id,
            whatsapp_account_id: model.whatsapp_account_id,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn to_active_model(&self) -> conversation_model::ActiveModel {
        use sea_orm::ActiveValue::NotSet;

        conversation_model::ActiveModel {
            id: if self.id == 0 { NotSet } else { Set(self.id) },
            contact_phone: Set(self.contact_phone.clone()),
            contact_name: Set(self.contact_name.clone()),
            last_message_at: Set(self.last_message_at),
            tenant_id: Set(self.tenant_id),
            contact_id: Set(self.contact_id),
            whatsapp_account_id: Set(self.whatsapp_account_id),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_conversation_requires_phone() {
        let result = Conversation::new(1, 1, "".into(), None, 1);
        assert!(matches!(result, Err(MessagingError::MissingField(_))));
    }

    #[test]
    fn new_conversation_accepts_valid_params() {
        let conv = Conversation::new(1, 1, "628996926184".into(), Some("Jane".into()), 1).unwrap();

        assert_eq!(conv.tenant_id(), Some(1));
        assert_eq!(conv.contact_id(), Some(1));
        assert_eq!(conv.contact_phone(), "628996926184");
        assert_eq!(conv.contact_name(), Some("Jane"));
        assert_eq!(conv.whatsapp_account_id(), Some(1));
        assert!(conv.last_message_at().is_none());
    }

    #[test]
    fn touch_updates_timestamps() {
        let conv = Conversation::new(1, 1, "628996926184".into(), None, 1).unwrap();
        let ts = chrono::Utc::now().naive_utc();
        let touched = conv.touch(ts);

        assert_eq!(touched.last_message_at(), Some(ts));
        assert_eq!(touched.updated_at(), ts);
    }
}
