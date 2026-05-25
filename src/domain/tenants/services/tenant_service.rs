use crate::domain::tenants::{
    StorageSettings, Tenant, TenantError, TenantRepository, WhatsAppSettings,
};

pub struct TenantService<R: TenantRepository> {
    repo: R,
}

impl<R: TenantRepository> TenantService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub async fn create_tenant(&self, name: String, slug: String) -> Result<Tenant, TenantError> {
        let tenant = Tenant::new(name, slug)?;
        self.repo.create_tenant(&tenant).await
    }

    pub async fn get_tenant(&self, tenant_id: i32) -> Result<Tenant, TenantError> {
        self.repo
            .find_tenant(tenant_id)
            .await?
            .ok_or(TenantError::NotFound)
    }

    pub async fn list_whatsapp_accounts(
        &self,
        tenant_id: i32,
    ) -> Result<Vec<WhatsAppSettings>, TenantError> {
        self.repo.list_whatsapp_accounts(tenant_id).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_whatsapp_account(
        &self,
        tenant_id: i32,
        phone_number_id: String,
        business_account_id: String,
        display_phone_number: Option<String>,
        access_token: String,
        verify_token: String,
        api_version: Option<String>,
        is_active: bool,
    ) -> Result<WhatsAppSettings, TenantError> {
        let settings = WhatsAppSettings::new(
            tenant_id,
            phone_number_id,
            business_account_id,
            display_phone_number,
            access_token,
            verify_token,
            api_version,
            is_active,
        )?;
        self.repo.create_whatsapp_account(&settings).await
    }

    pub async fn get_storage_config(
        &self,
        tenant_id: i32,
    ) -> Result<Option<StorageSettings>, TenantError> {
        self.repo.get_storage_config(tenant_id).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_storage_config(
        &self,
        tenant_id: i32,
        endpoint: String,
        region: Option<String>,
        access_key_id: String,
        secret_access_key: String,
        bucket: String,
        public_base_url: Option<String>,
        is_active: bool,
    ) -> Result<StorageSettings, TenantError> {
        let settings = StorageSettings::new(
            tenant_id,
            endpoint,
            region,
            access_key_id,
            secret_access_key,
            bucket,
            public_base_url,
            is_active,
        )?;
        self.repo.create_storage_config(&settings).await
    }
}
