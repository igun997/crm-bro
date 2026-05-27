use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use crate::{
    domain::contacts::{Contact, ContactError, ContactRepository, Pagination},
    infrastructure::persistence::models::contact,
};

pub struct SeaOrmContactRepository {
    db: DatabaseConnection,
}

impl SeaOrmContactRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ContactRepository for SeaOrmContactRepository {
    async fn find_by_id(&self, id: i32, tenant_id: i32) -> Result<Option<Contact>, ContactError> {
        contact::Entity::find()
            .filter(contact::Column::Id.eq(id))
            .filter(contact::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map(|model| model.map(Contact::from_model))
            .map_err(|error| ContactError::Database(error.to_string()))
    }

    async fn find_by_phone(
        &self,
        phone: &str,
        tenant_id: i32,
    ) -> Result<Option<Contact>, ContactError> {
        contact::Entity::find()
            .filter(contact::Column::Phone.eq(phone))
            .filter(contact::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map(|model| model.map(Contact::from_model))
            .map_err(|error| ContactError::Database(error.to_string()))
    }

    async fn list(
        &self,
        tenant_id: i32,
        pagination: Pagination,
    ) -> Result<Vec<Contact>, ContactError> {
        let pagination = pagination.normalized();

        contact::Entity::find()
            .filter(contact::Column::TenantId.eq(tenant_id))
            .order_by_desc(contact::Column::UpdatedAt)
            .paginate(&self.db, pagination.limit)
            .fetch_page(pagination.offset / pagination.limit)
            .await
            .map(|models| models.into_iter().map(Contact::from_model).collect())
            .map_err(|error| ContactError::Database(error.to_string()))
    }

    async fn save(&self, contact: &Contact) -> Result<Contact, ContactError> {
        let active = contact.to_active_model();
        if contact.id() == 0 {
            contact::Entity::insert(active)
                .exec_with_returning(&self.db)
                .await
                .map(Contact::from_model)
                .map_err(|error| ContactError::Database(error.to_string()))
        } else {
            contact::Entity::update(active)
                .exec(&self.db)
                .await
                .map(Contact::from_model)
                .map_err(|error| ContactError::Database(error.to_string()))
        }
    }

    async fn delete(&self, id: i32, tenant_id: i32) -> Result<(), ContactError> {
        contact::Entity::delete_many()
            .filter(contact::Column::Id.eq(id))
            .filter(contact::Column::TenantId.eq(tenant_id))
            .exec(&self.db)
            .await
            .map(|_| ())
            .map_err(|error| ContactError::Database(error.to_string()))
    }
}
