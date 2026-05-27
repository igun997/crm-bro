use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};

use crate::{
    domain::tenants::{StorageSettings, Tenant, TenantError, TenantRepository, WhatsAppSettings},
    infrastructure::persistence::models::{tenant, tenant_storage_config, tenant_whatsapp_account},
};

pub struct SeaOrmTenantRepository {
    db: DatabaseConnection,
}

impl SeaOrmTenantRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TenantRepository for SeaOrmTenantRepository {
    async fn find_tenant(&self, tenant_id: i32) -> Result<Option<Tenant>, TenantError> {
        tenant::Entity::find_by_id(tenant_id)
            .one(&self.db)
            .await
            .map(|model| model.map(Tenant::from_model))
            .map_err(|error| TenantError::Database(error.to_string()))
    }

    async fn create_tenant(&self, tenant: &Tenant) -> Result<Tenant, TenantError> {
        tenant::ActiveModel {
            name: Set(tenant.name().to_string()),
            slug: Set(tenant.slug().to_string()),
            is_active: Set(tenant.is_active()),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map(Tenant::from_model)
        .map_err(|error| TenantError::Database(error.to_string()))
    }

    async fn list_whatsapp_accounts(
        &self,
        tenant_id: i32,
    ) -> Result<Vec<WhatsAppSettings>, TenantError> {
        tenant_whatsapp_account::Entity::find()
            .filter(tenant_whatsapp_account::Column::TenantId.eq(tenant_id))
            .all(&self.db)
            .await
            .map(|models| {
                models
                    .into_iter()
                    .map(WhatsAppSettings::from_model)
                    .collect()
            })
            .map_err(|error| TenantError::Database(error.to_string()))
    }

    async fn create_whatsapp_account(
        &self,
        settings: &WhatsAppSettings,
    ) -> Result<WhatsAppSettings, TenantError> {
        tenant_whatsapp_account::ActiveModel {
            tenant_id: Set(settings.tenant_id()),
            phone_number_id: Set(settings.phone_number_id().to_string()),
            business_account_id: Set(settings.business_account_id().to_string()),
            display_phone_number: Set(settings.display_phone_number().map(ToString::to_string)),
            access_token: Set(settings.access_token().to_string()),
            verify_token: Set(settings.verify_token().to_string()),
            api_version: Set(settings.api_version().to_string()),
            is_active: Set(settings.is_active()),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map(WhatsAppSettings::from_model)
        .map_err(|error| TenantError::Database(error.to_string()))
    }

    async fn get_storage_config(
        &self,
        tenant_id: i32,
    ) -> Result<Option<StorageSettings>, TenantError> {
        tenant_storage_config::Entity::find()
            .filter(tenant_storage_config::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map(|model| model.map(StorageSettings::from_model))
            .map_err(|error| TenantError::Database(error.to_string()))
    }

    async fn create_storage_config(
        &self,
        settings: &StorageSettings,
    ) -> Result<StorageSettings, TenantError> {
        tenant_storage_config::ActiveModel {
            tenant_id: Set(settings.tenant_id()),
            endpoint: Set(settings.endpoint().to_string()),
            region: Set(settings.region().to_string()),
            access_key_id: Set(settings.access_key_id().to_string()),
            secret_access_key: Set(settings.secret_access_key().to_string()),
            bucket: Set(settings.bucket().to_string()),
            public_base_url: Set(settings.public_base_url().map(ToString::to_string)),
            is_active: Set(settings.is_active()),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .map(StorageSettings::from_model)
        .map_err(|error| TenantError::Database(error.to_string()))
    }
}
