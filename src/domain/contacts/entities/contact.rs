use chrono::{NaiveDateTime, Utc};
use sea_orm::{ActiveValue::NotSet, Set};

use crate::{domain::contacts::errors::ContactError, models::contact};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    id: i32,
    tenant_id: i32,
    name: Option<String>,
    phone: String,
    email: Option<String>,
    company: Option<String>,
    notes: Option<String>,
    owner_user_id: Option<i32>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

impl Contact {
    pub fn new(tenant_id: i32, name: String, phone: String) -> Result<Self, ContactError> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(ContactError::InvalidName("name cannot be empty".into()));
        }

        if !Self::validate_phone(&phone) {
            return Err(ContactError::InvalidPhone(phone));
        }

        let now = Utc::now().naive_utc();
        Ok(Self {
            id: 0,
            tenant_id,
            name: Some(name),
            phone: Self::normalize_phone(&phone),
            email: None,
            company: None,
            notes: None,
            owner_user_id: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn tenant_id(&self) -> i32 {
        self.tenant_id
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn phone(&self) -> &str {
        &self.phone
    }

    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    pub fn company(&self) -> Option<&str> {
        self.company.as_deref()
    }

    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    pub fn owner_user_id(&self) -> Option<i32> {
        self.owner_user_id
    }

    pub fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    pub fn updated_at(&self) -> NaiveDateTime {
        self.updated_at
    }

    pub(crate) fn from_model(model: contact::Model) -> Self {
        Self {
            id: model.id,
            tenant_id: model.tenant_id,
            name: model.name,
            phone: model.phone,
            email: model.email,
            company: model.company,
            notes: model.notes,
            owner_user_id: model.owner_user_id,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }

    pub(crate) fn to_active_model(&self) -> contact::ActiveModel {
        contact::ActiveModel {
            id: if self.id == 0 { NotSet } else { Set(self.id) },
            tenant_id: Set(self.tenant_id),
            phone: Set(self.phone.clone()),
            name: Set(self.name.clone()),
            email: Set(self.email.clone()),
            company: Set(self.company.clone()),
            notes: Set(self.notes.clone()),
            owner_user_id: Set(self.owner_user_id),
            created_at: if self.id == 0 {
                NotSet
            } else {
                Set(self.created_at)
            },
            updated_at: Set(self.updated_at),
        }
    }

    fn validate_phone(phone: &str) -> bool {
        let trimmed = phone.trim();
        let digits = trimmed.chars().filter(|c| c.is_ascii_digit()).count();
        digits >= 10
            && trimmed
                .chars()
                .all(|c| c.is_ascii_digit() || c == '+' || c.is_whitespace() || c == '-')
    }

    fn normalize_phone(phone: &str) -> String {
        phone.chars().filter(|c| c.is_ascii_digit()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_contact_rejects_empty_name() {
        let result = Contact::new(1, "  ".into(), "628996926184".into());
        assert!(matches!(result, Err(ContactError::InvalidName(_))));
    }

    #[test]
    fn new_contact_rejects_invalid_phone() {
        let result = Contact::new(1, "Jane".into(), "abc".into());
        assert!(matches!(result, Err(ContactError::InvalidPhone(_))));
    }

    #[test]
    fn new_contact_normalizes_phone() {
        let contact = Contact::new(1, "Jane".into(), "+62 899-692-6184".into()).unwrap();
        assert_eq!(contact.phone(), "628996926184");
    }

    #[test]
    fn from_model_maps_all_fields() {
        let now = Utc::now().naive_utc();
        let model = contact::Model {
            id: 7,
            tenant_id: 1,
            phone: "628996926184".into(),
            name: Some("Jane".into()),
            email: Some("jane@example.com".into()),
            company: Some("Acme".into()),
            notes: Some("VIP".into()),
            owner_user_id: Some(9),
            created_at: now,
            updated_at: now,
        };

        let contact = Contact::from_model(model);

        assert_eq!(contact.id(), 7);
        assert_eq!(contact.tenant_id(), 1);
        assert_eq!(contact.name(), Some("Jane"));
        assert_eq!(contact.phone(), "628996926184");
        assert_eq!(contact.email(), Some("jane@example.com"));
        assert_eq!(contact.company(), Some("Acme"));
        assert_eq!(contact.notes(), Some("VIP"));
        assert_eq!(contact.owner_user_id(), Some(9));
    }

    #[test]
    fn to_active_model_omits_id_for_new_contact() {
        let contact = Contact::new(1, "Jane".into(), "628996926184".into()).unwrap();
        let active = contact.to_active_model();

        assert!(matches!(active.id, NotSet));
    }
}
