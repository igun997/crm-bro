use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tenant_whatsapp_accounts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub tenant_id: i32,
    #[sea_orm(unique)]
    pub phone_number_id: String,
    pub business_account_id: String,
    pub display_phone_number: Option<String>,
    pub access_token: String,
    pub verify_token: String,
    pub api_version: String,
    pub is_active: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tenant::Entity",
        from = "Column::TenantId",
        to = "super::tenant::Column::Id"
    )]
    Tenant,
}

impl ActiveModelBehavior for ActiveModel {}
