use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "messages")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub conversation_id: i32,
    #[sea_orm(unique)]
    pub wa_message_id: Option<String>,
    pub direction: String,
    pub msg_type: String,
    pub body: Option<String>,
    pub media_url: Option<String>,
    pub media_mime: Option<String>,
    pub template_name: Option<String>,
    pub status: String,
    pub timestamp: chrono::NaiveDateTime,
    pub tenant_id: Option<i32>,
    pub contact_id: Option<i32>,
    pub storage_key: Option<String>,
    pub original_filename: Option<String>,
    pub size_bytes: Option<i64>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::conversation::Entity",
        from = "Column::ConversationId",
        to = "super::conversation::Column::Id"
    )]
    Conversation,
    #[sea_orm(belongs_to = "super::tenant::Entity", from = "Column::TenantId", to = "super::tenant::Column::Id")]
    Tenant,
    #[sea_orm(belongs_to = "super::contact::Entity", from = "Column::ContactId", to = "super::contact::Column::Id")]
    Contact,
}

impl Related<super::conversation::Entity> for Entity {
    fn to() -> RelationDef { Relation::Conversation.def() }
}

impl ActiveModelBehavior for ActiveModel {}
