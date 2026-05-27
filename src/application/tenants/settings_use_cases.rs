use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};

use crate::domain::tenants::{
    SeaOrmTenantRepository, StorageSettings, TenantService, WhatsAppSettings,
};
use crate::infrastructure::persistence::models::{tenant_storage_config, tenant_whatsapp_account};

pub struct WhatsAppAccountsResult {
    pub tenant_slug: String,
    pub accounts: Vec<WhatsAppSettings>,
}

pub struct WhatsAppAccountResult {
    pub tenant_slug: String,
    pub account: WhatsAppSettings,
}

pub struct CreateWhatsAppAccountInput {
    pub tenant_id: i32,
    pub phone_number_id: String,
    pub business_account_id: String,
    pub display_phone_number: Option<String>,
    pub access_token: String,
    pub verify_token: String,
    pub api_version: Option<String>,
    pub is_active: bool,
}

pub struct PatchWhatsAppAccountInput {
    pub id: i32,
    pub tenant_id: i32,
    pub phone_number_id: Option<String>,
    pub business_account_id: Option<String>,
    pub display_phone_number: Option<String>,
    pub access_token: Option<String>,
    pub verify_token: Option<String>,
    pub api_version: Option<String>,
    pub is_active: Option<bool>,
}

pub struct CreateStorageConfigInput {
    pub tenant_id: i32,
    pub endpoint: String,
    pub region: Option<String>,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
    pub public_base_url: Option<String>,
    pub is_active: bool,
}

pub struct PatchStorageConfigInput {
    pub tenant_id: i32,
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub bucket: Option<String>,
    pub public_base_url: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn list_whatsapp_accounts(
    db: &DatabaseConnection,
    tenant_id: i32,
) -> Result<WhatsAppAccountsResult, sea_orm::DbErr> {
    let service = TenantService::new(SeaOrmTenantRepository::new(db.clone()));
    let tenant = service.get_tenant(tenant_id).await.map_err(to_db_err)?;
    let accounts = service
        .list_whatsapp_accounts(tenant_id)
        .await
        .map_err(to_db_err)?;
    Ok(WhatsAppAccountsResult {
        tenant_slug: tenant.slug().to_string(),
        accounts,
    })
}

pub async fn create_whatsapp_account(
    db: &DatabaseConnection,
    input: CreateWhatsAppAccountInput,
) -> Result<WhatsAppAccountResult, sea_orm::DbErr> {
    let service = TenantService::new(SeaOrmTenantRepository::new(db.clone()));
    let tenant = service
        .get_tenant(input.tenant_id)
        .await
        .map_err(to_db_err)?;
    let account = service
        .create_whatsapp_account(
            input.tenant_id,
            input.phone_number_id,
            input.business_account_id,
            input.display_phone_number,
            input.access_token,
            input.verify_token,
            input.api_version,
            input.is_active,
        )
        .await
        .map_err(to_db_err)?;
    Ok(WhatsAppAccountResult {
        tenant_slug: tenant.slug().to_string(),
        account,
    })
}

pub async fn update_whatsapp_account(
    db: &DatabaseConnection,
    input: PatchWhatsAppAccountInput,
) -> Result<Option<WhatsAppAccountResult>, sea_orm::DbErr> {
    let result = list_whatsapp_accounts(db, input.tenant_id).await?;
    let account = tenant_whatsapp_account::Entity::find()
        .filter(tenant_whatsapp_account::Column::Id.eq(input.id))
        .filter(tenant_whatsapp_account::Column::TenantId.eq(input.tenant_id))
        .one(db)
        .await?;
    let Some(account) = account else {
        return Ok(None);
    };

    let mut active: tenant_whatsapp_account::ActiveModel = account.into();
    if let Some(value) = input.phone_number_id {
        active.phone_number_id = Set(value);
    }
    if let Some(value) = input.business_account_id {
        active.business_account_id = Set(value);
    }
    if let Some(value) = input.display_phone_number {
        active.display_phone_number = Set(Some(value));
    }
    if let Some(value) = input.access_token {
        active.access_token = Set(value);
    }
    if let Some(value) = input.verify_token {
        active.verify_token = Set(value);
    }
    if let Some(value) = input.api_version {
        active.api_version = Set(value);
    }
    if let Some(value) = input.is_active {
        active.is_active = Set(value);
    }

    let account = active.update(db).await?;
    Ok(Some(WhatsAppAccountResult {
        tenant_slug: result.tenant_slug,
        account: WhatsAppSettings::from_model(account),
    }))
}

pub async fn get_storage_config(
    db: &DatabaseConnection,
    tenant_id: i32,
) -> Result<Option<StorageSettings>, sea_orm::DbErr> {
    let service = TenantService::new(SeaOrmTenantRepository::new(db.clone()));
    service
        .get_storage_config(tenant_id)
        .await
        .map_err(to_db_err)
}

pub async fn create_storage_config(
    db: &DatabaseConnection,
    input: CreateStorageConfigInput,
) -> Result<StorageSettings, sea_orm::DbErr> {
    let service = TenantService::new(SeaOrmTenantRepository::new(db.clone()));
    service
        .create_storage_config(
            input.tenant_id,
            input.endpoint,
            input.region,
            input.access_key_id,
            input.secret_access_key,
            input.bucket,
            input.public_base_url,
            input.is_active,
        )
        .await
        .map_err(to_db_err)
}

pub async fn update_storage_config(
    db: &DatabaseConnection,
    input: PatchStorageConfigInput,
) -> Result<Option<StorageSettings>, sea_orm::DbErr> {
    let config = tenant_storage_config::Entity::find()
        .filter(tenant_storage_config::Column::TenantId.eq(input.tenant_id))
        .one(db)
        .await?;
    let Some(config) = config else {
        return Ok(None);
    };

    let mut active: tenant_storage_config::ActiveModel = config.into();
    if let Some(value) = input.endpoint {
        active.endpoint = Set(value);
    }
    if let Some(value) = input.region {
        active.region = Set(value);
    }
    if let Some(value) = input.access_key_id {
        active.access_key_id = Set(value);
    }
    if let Some(value) = input.secret_access_key {
        active.secret_access_key = Set(value);
    }
    if let Some(value) = input.bucket {
        active.bucket = Set(value);
    }
    if let Some(value) = input.public_base_url {
        active.public_base_url = Set(Some(value));
    }
    if let Some(value) = input.is_active {
        active.is_active = Set(value);
    }

    let config = active.update(db).await?;
    Ok(Some(StorageSettings::from_model(config)))
}

fn to_db_err(error: impl std::fmt::Display) -> sea_orm::DbErr {
    sea_orm::DbErr::Custom(error.to_string())
}
