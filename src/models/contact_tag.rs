use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "contact_tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub contact_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub tag_id: i32,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(belongs_to = "super::contact::Entity", from = "Column::ContactId", to = "super::contact::Column::Id")]
    Contact,
    #[sea_orm(belongs_to = "super::tag::Entity", from = "Column::TagId", to = "super::tag::Column::Id")]
    Tag,
}

impl ActiveModelBehavior for ActiveModel {}
