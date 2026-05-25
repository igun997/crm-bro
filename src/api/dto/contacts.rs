use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"id": 1, "tenant_id": 1, "name": "VIP", "color": "#ff0000"}))]
pub struct TagResponse {
    pub id: i32,
    pub tenant_id: i32,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "id": 1, "tenant_id": 1, "phone": "628123456789",
    "name": "Alice", "email": "alice@example.com", "company": "Acme",
    "notes": "Key decision maker", "owner_user_id": 2,
    "tags": [{"id": 1, "tenant_id": 1, "name": "VIP", "color": "#ff0000"}]
}))]
pub struct ContactResponse {
    pub id: i32,
    pub tenant_id: i32,
    pub phone: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub company: Option<String>,
    pub notes: Option<String>,
    pub owner_user_id: Option<i32>,
    pub tags: Vec<TagResponse>,
}

impl From<crate::domain::contacts::Contact> for ContactResponse {
    fn from(contact: crate::domain::contacts::Contact) -> Self {
        Self {
            id: contact.id(),
            tenant_id: contact.tenant_id(),
            phone: contact.phone().to_owned(),
            name: contact.name().map(str::to_owned),
            email: contact.email().map(str::to_owned),
            company: contact.company().map(str::to_owned),
            notes: contact.notes().map(str::to_owned),
            owner_user_id: contact.owner_user_id(),
            tags: vec![],
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ContactListQuery {
    pub q: Option<String>,
    pub tag: Option<String>,
    pub owner_user_id: Option<i32>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"data": [], "page": 1, "per_page": 20, "total": 0}))]
pub struct PaginatedContacts {
    pub data: Vec<ContactResponse>,
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"name": "Alice Updated", "company": "Acme Inc"}))]
pub struct PatchContactRequest {
    pub name: Option<String>,
    pub email: Option<String>,
    pub company: Option<String>,
    pub notes: Option<String>,
    pub owner_user_id: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"name": "VIP", "color": "#ff0000"}))]
pub struct CreateTagRequest {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"tag_id": 1}))]
pub struct AttachTagRequest {
    pub tag_id: Option<i32>,
    pub name: Option<String>,
    pub color: Option<String>,
}
