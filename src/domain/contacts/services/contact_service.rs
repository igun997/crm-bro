use crate::domain::contacts::{Contact, ContactError, ContactRepository, Pagination};

pub struct ContactService<R: ContactRepository> {
    repo: R,
}

impl<R: ContactRepository> ContactService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub async fn create(
        &self,
        tenant_id: i32,
        name: String,
        phone: String,
    ) -> Result<Contact, ContactError> {
        let contact = Contact::new(tenant_id, name, phone)?;

        if self
            .repo
            .find_by_phone(contact.phone(), tenant_id)
            .await?
            .is_some()
        {
            return Err(ContactError::DuplicatePhone(contact.phone().to_string()));
        }

        self.repo.save(&contact).await
    }

    pub async fn get(&self, id: i32, tenant_id: i32) -> Result<Contact, ContactError> {
        self.repo
            .find_by_id(id, tenant_id)
            .await?
            .ok_or(ContactError::NotFound(id))
    }

    pub async fn list(
        &self,
        tenant_id: i32,
        pagination: Pagination,
    ) -> Result<Vec<Contact>, ContactError> {
        self.repo.list(tenant_id, pagination).await
    }

    pub async fn delete(&self, id: i32, tenant_id: i32) -> Result<(), ContactError> {
        self.repo.delete(id, tenant_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mockall::{mock, predicate::*};

    mock! {
        Repo {}

        #[async_trait]
        impl ContactRepository for Repo {
            async fn find_by_id(&self, id: i32, tenant_id: i32) -> Result<Option<Contact>, ContactError>;
            async fn find_by_phone(&self, phone: &str, tenant_id: i32) -> Result<Option<Contact>, ContactError>;
            async fn list(&self, tenant_id: i32, pagination: Pagination) -> Result<Vec<Contact>, ContactError>;
            async fn save(&self, contact: &Contact) -> Result<Contact, ContactError>;
            async fn delete(&self, id: i32, tenant_id: i32) -> Result<(), ContactError>;
        }
    }

    #[tokio::test]
    async fn create_returns_duplicate_phone() {
        let existing = Contact::new(1, "Jane".into(), "628996926184".into()).unwrap();
        let mut repo = MockRepo::new();
        repo.expect_find_by_phone()
            .with(eq("628996926184"), eq(1))
            .times(1)
            .returning(move |_, _| Ok(Some(existing.clone())));
        repo.expect_save().times(0);

        let service = ContactService::new(repo);
        let result = service
            .create(1, "Jane".into(), "628996926184".into())
            .await;

        assert!(matches!(result, Err(ContactError::DuplicatePhone(_))));
    }

    #[tokio::test]
    async fn create_invalid_phone_does_not_call_save() {
        let mut repo = MockRepo::new();
        repo.expect_find_by_phone().times(0);
        repo.expect_save().times(0);

        let service = ContactService::new(repo);
        let result = service.create(1, "Jane".into(), "abc".into()).await;

        assert!(matches!(result, Err(ContactError::InvalidPhone(_))));
    }

    #[tokio::test]
    async fn create_valid_contact_calls_save() {
        let mut repo = MockRepo::new();
        repo.expect_find_by_phone()
            .with(eq("628996926184"), eq(1))
            .times(1)
            .returning(|_, _| Ok(None));
        repo.expect_save().times(1).returning(|contact| Ok(contact.clone()));

        let service = ContactService::new(repo);
        let contact = service
            .create(1, "Jane".into(), "+62 899-692-6184".into())
            .await
            .unwrap();

        assert_eq!(contact.phone(), "628996926184");
    }
}
