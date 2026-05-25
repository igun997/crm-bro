use async_trait::async_trait;

use crate::domain::contacts::{Contact, ContactError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pagination {
    pub limit: u64,
    pub offset: u64,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
        }
    }
}

#[async_trait]
pub trait ContactRepository: Send + Sync {
    async fn find_by_id(&self, id: i32, tenant_id: i32) -> Result<Option<Contact>, ContactError>;
    async fn find_by_phone(
        &self,
        phone: &str,
        tenant_id: i32,
    ) -> Result<Option<Contact>, ContactError>;
    async fn list(
        &self,
        tenant_id: i32,
        pagination: Pagination,
    ) -> Result<Vec<Contact>, ContactError>;
    async fn save(&self, contact: &Contact) -> Result<Contact, ContactError>;
    async fn delete(&self, id: i32, tenant_id: i32) -> Result<(), ContactError>;
}
