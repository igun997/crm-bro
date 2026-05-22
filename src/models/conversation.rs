use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "conversations")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub contact_phone: String,
    pub contact_name: Option<String>,
    pub last_message_at: Option<chrono::NaiveDateTime>,
    pub tenant_id: Option<i32>,
    pub contact_id: Option<i32>,
    pub whatsapp_account_id: Option<i32>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::message::Entity")]
    Messages,
    #[sea_orm(belongs_to = "super::tenant::Entity", from = "Column::TenantId", to = "super::tenant::Column::Id")]
    Tenant,
    #[sea_orm(belongs_to = "super::contact::Entity", from = "Column::ContactId", to = "super::contact::Column::Id")]
    Contact,
    #[sea_orm(belongs_to = "super::tenant_whatsapp_account::Entity", from = "Column::WhatsappAccountId", to = "super::tenant_whatsapp_account::Column::Id")]
    WhatsappAccount,
}

impl Related<super::message::Entity> for Entity {
    fn to() -> RelationDef { Relation::Messages.def() }
}

impl ActiveModelBehavior for ActiveModel {}
