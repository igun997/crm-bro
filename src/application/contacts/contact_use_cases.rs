use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
};

use crate::infrastructure::persistence::models::{contact, contact_tag, tag};

#[derive(Debug, Clone)]
pub struct ListContactsInput {
    pub tenant_id: i32,
    pub q: Option<String>,
    pub tag: Option<String>,
    pub owner_user_id: Option<i32>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone)]
pub struct ListContactsOutput {
    pub contacts: Vec<contact::Model>,
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
}

pub async fn list_contacts(
    db: &DatabaseConnection,
    input: ListContactsInput,
) -> Result<ListContactsOutput, sea_orm::DbErr> {
    let mut condition = Condition::all().add(contact::Column::TenantId.eq(input.tenant_id));
    if let Some(owner_user_id) = input.owner_user_id {
        condition = condition.add(contact::Column::OwnerUserId.eq(owner_user_id));
    }
    if let Some(q) = input.q.as_ref().filter(|q| !q.trim().is_empty()) {
        let pattern = format!("%{}%", q.trim());
        condition = condition.add(
            Condition::any()
                .add(contact::Column::Phone.like(pattern.clone()))
                .add(contact::Column::Name.like(pattern.clone()))
                .add(contact::Column::Email.like(pattern.clone()))
                .add(contact::Column::Company.like(pattern.clone()))
                .add(contact::Column::Notes.like(pattern)),
        );
    }

    if let Some(tag_name) = input.tag.as_ref().filter(|name| !name.trim().is_empty()) {
        let ids = contact_ids_for_tag(db, input.tenant_id, tag_name.trim()).await?;
        if ids.is_empty() {
            return Ok(ListContactsOutput {
                contacts: vec![],
                page: input.page,
                per_page: input.per_page,
                total: 0,
            });
        }
        condition = condition.add(contact::Column::Id.is_in(ids));
    }

    let paginator = contact::Entity::find()
        .filter(condition)
        .paginate(db, input.per_page);
    let total = paginator.num_items().await?;
    let contacts = paginator.fetch_page(input.page - 1).await?;

    Ok(ListContactsOutput {
        contacts,
        page: input.page,
        per_page: input.per_page,
        total,
    })
}

async fn contact_ids_for_tag(
    db: &DatabaseConnection,
    tenant_id: i32,
    tag_name: &str,
) -> Result<Vec<i32>, sea_orm::DbErr> {
    let tag = tag::Entity::find()
        .filter(tag::Column::TenantId.eq(tenant_id))
        .filter(tag::Column::Name.eq(tag_name))
        .one(db)
        .await?;
    let Some(tag) = tag else {
        return Ok(vec![]);
    };
    let links = contact_tag::Entity::find()
        .filter(contact_tag::Column::TagId.eq(tag.id))
        .all(db)
        .await?;
    Ok(links.into_iter().map(|link| link.contact_id).collect())
}
