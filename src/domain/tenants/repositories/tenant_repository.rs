use async_trait::async_trait;

use crate::domain::tenants::{StorageSettings, Tenant, TenantError, WhatsAppSettings};

#[async_trait]
pub trait TenantRepository: Send + Sync {
    async fn find_tenant(&self, tenant_id: i32) -> Result<Option<Tenant>, TenantError>;
    async fn create_tenant(&self, tenant: &Tenant) -> Result<Tenant, TenantError>;
    async fn list_whatsapp_accounts(
        &self,
        tenant_id: i32,
    ) -> Result<Vec<WhatsAppSettings>, TenantError>;
    async fn create_whatsapp_account(
        &self,
        settings: &WhatsAppSettings,
    ) -> Result<WhatsAppSettings, TenantError>;
    async fn get_storage_config(
        &self,
        tenant_id: i32,
    ) -> Result<Option<StorageSettings>, TenantError>;
    async fn create_storage_config(
        &self,
        settings: &StorageSettings,
    ) -> Result<StorageSettings, TenantError>;
}
