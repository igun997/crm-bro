use chrono::{NaiveDateTime, Utc};

use crate::domain::contacts::errors::ContactError;

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
}
