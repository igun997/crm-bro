use actix_web::{delete, get, patch, post, web, HttpResponse};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::auth::CurrentUser;
use crate::models::{contact, contact_tag, tag, user};

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

#[utoipa::path(
    get,
    path = "/api/contacts",
    params(ContactListQuery),
    responses((status = 200, description = "Contacts", body = PaginatedContacts)),
    tag = "Contacts"
)]
#[get("/contacts")]
pub async fn list_contacts(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    query: web::Query<ContactListQuery>,
) -> HttpResponse {
    let Some(tenant_id) = current.0.tenant_id else {
        return tenant_required();
    };
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);

    let mut condition = Condition::all().add(contact::Column::TenantId.eq(tenant_id));
    if let Some(owner_user_id) = query.owner_user_id {
        condition = condition.add(contact::Column::OwnerUserId.eq(owner_user_id));
    }
    if let Some(q) = query.q.as_ref().filter(|q| !q.trim().is_empty()) {
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

    let tag_contact_ids =
        if let Some(tag_name) = query.tag.as_ref().filter(|name| !name.trim().is_empty()) {
            match contact_ids_for_tag(db.get_ref(), tenant_id, tag_name.trim()).await {
                Ok(ids) => Some(ids),
                Err(response) => return response,
            }
        } else {
            None
        };
    if let Some(ids) = tag_contact_ids {
        if ids.is_empty() {
            return HttpResponse::Ok().json(PaginatedContacts {
                data: vec![],
                page,
                per_page,
                total: 0,
            });
        }
        condition = condition.add(contact::Column::Id.is_in(ids));
    }

    let paginator = contact::Entity::find()
        .filter(condition)
        .paginate(db.get_ref(), per_page);
    let total = match paginator.num_items().await {
        Ok(total) => total,
        Err(error) => {
            tracing::error!(%error, "Failed to count contacts");
            return server_error("Failed to list contacts");
        }
    };
    let contacts = match paginator.fetch_page(page - 1).await {
        Ok(contacts) => contacts,
        Err(error) => {
            tracing::error!(%error, "Failed to fetch contacts");
            return server_error("Failed to list contacts");
        }
    };

    match contact_responses(db.get_ref(), contacts).await {
        Ok(data) => HttpResponse::Ok().json(PaginatedContacts {
            data,
            page,
            per_page,
            total,
        }),
        Err(response) => response,
    }
}

#[utoipa::path(
    get,
    path = "/api/contacts/{id}",
    params(("id" = i32, Path, description = "Contact id")),
    responses((status = 200, description = "Contact", body = ContactResponse)),
    tag = "Contacts"
)]
#[get("/contacts/{id}")]
pub async fn get_contact(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
) -> HttpResponse {
    let Some(tenant_id) = current.0.tenant_id else {
        return tenant_required();
    };
    match load_contact(db.get_ref(), tenant_id, path.into_inner()).await {
        Ok(Some(contact)) => match contact_response(db.get_ref(), contact).await {
            Ok(response) => HttpResponse::Ok().json(response),
            Err(response) => response,
        },
        Ok(None) => not_found("Contact not found"),
        Err(response) => response,
    }
}

#[utoipa::path(
    patch,
    path = "/api/contacts/{id}",
    request_body = PatchContactRequest,
    params(("id" = i32, Path, description = "Contact id")),
    responses((status = 200, description = "Contact updated", body = ContactResponse)),
    tag = "Contacts"
)]
#[patch("/contacts/{id}")]
pub async fn update_contact(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
    body: web::Json<PatchContactRequest>,
) -> HttpResponse {
    let Some(tenant_id) = current.0.tenant_id else {
        return tenant_required();
    };
    let contact = match load_contact(db.get_ref(), tenant_id, path.into_inner()).await {
        Ok(Some(contact)) => contact,
        Ok(None) => return not_found("Contact not found"),
        Err(response) => return response,
    };
    let mut active: contact::ActiveModel = contact.into();
    if let Some(value) = &body.name {
        active.name = Set(Some(value.clone()));
    }
    if let Some(value) = &body.email {
        active.email = Set(Some(value.clone()));
    }
    if let Some(value) = &body.company {
        active.company = Set(Some(value.clone()));
    }
    if let Some(value) = &body.notes {
        active.notes = Set(Some(value.clone()));
    }
    if let Some(value) = body.owner_user_id {
        if !user_belongs_to_tenant(db.get_ref(), tenant_id, value).await {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": "owner_user_id must belong to current tenant"
            }));
        }
        active.owner_user_id = Set(Some(value));
    }

    match active.update(db.get_ref()).await {
        Ok(contact) => match contact_response(db.get_ref(), contact).await {
            Ok(response) => HttpResponse::Ok().json(response),
            Err(response) => response,
        },
        Err(error) => {
            tracing::error!(%error, "Failed to update contact");
            server_error("Failed to update contact")
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/contacts/{id}/tags",
    request_body = AttachTagRequest,
    params(("id" = i32, Path, description = "Contact id")),
    responses((status = 200, description = "Contact", body = ContactResponse)),
    tag = "Contacts"
)]
#[post("/contacts/{id}/tags")]
pub async fn attach_tag(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
    body: web::Json<AttachTagRequest>,
) -> HttpResponse {
    let Some(tenant_id) = current.0.tenant_id else {
        return tenant_required();
    };
    let contact_id = path.into_inner();
    let contact = match load_contact(db.get_ref(), tenant_id, contact_id).await {
        Ok(Some(contact)) => contact,
        Ok(None) => return not_found("Contact not found"),
        Err(response) => return response,
    };
    let tag = match resolve_or_create_tag(db.get_ref(), tenant_id, &body).await {
        Ok(tag) => tag,
        Err(response) => return response,
    };

    let exists = contact_tag::Entity::find()
        .filter(contact_tag::Column::ContactId.eq(contact_id))
        .filter(contact_tag::Column::TagId.eq(tag.id))
        .one(db.get_ref())
        .await;
    match exists {
        Ok(Some(_)) => {}
        Ok(None) => {
            if let Err(error) = (contact_tag::ActiveModel {
                contact_id: Set(contact_id),
                tag_id: Set(tag.id),
                ..Default::default()
            })
            .insert(db.get_ref())
            .await
            {
                tracing::error!(%error, "Failed to attach tag");
                return server_error("Failed to attach tag");
            }
        }
        Err(error) => {
            tracing::error!(%error, "Failed to check contact tag");
            return server_error("Failed to attach tag");
        }
    }

    match contact_response(db.get_ref(), contact).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(response) => response,
    }
}

#[utoipa::path(
    delete,
    path = "/api/contacts/{id}/tags/{tag_id}",
    params(("id" = i32, Path, description = "Contact id"), ("tag_id" = i32, Path, description = "Tag id")),
    responses((status = 200, description = "Tag detached")),
    tag = "Contacts"
)]
#[delete("/contacts/{id}/tags/{tag_id}")]
pub async fn detach_tag(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<(i32, i32)>,
) -> HttpResponse {
    let Some(tenant_id) = current.0.tenant_id else {
        return tenant_required();
    };
    let (contact_id, tag_id) = path.into_inner();
    if !contact_exists(db.get_ref(), tenant_id, contact_id).await {
        return not_found("Contact not found");
    }
    if !tag_exists(db.get_ref(), tenant_id, tag_id).await {
        return not_found("Tag not found");
    }

    match contact_tag::Entity::delete_many()
        .filter(contact_tag::Column::ContactId.eq(contact_id))
        .filter(contact_tag::Column::TagId.eq(tag_id))
        .exec(db.get_ref())
        .await
    {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"success": true})),
        Err(error) => {
            tracing::error!(%error, "Failed to detach tag");
            server_error("Failed to detach tag")
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/tags",
    responses((status = 200, description = "Tags", body = [TagResponse])),
    tag = "Contacts"
)]
#[get("/tags")]
pub async fn list_tags(current: CurrentUser, db: web::Data<DatabaseConnection>) -> HttpResponse {
    let Some(tenant_id) = current.0.tenant_id else {
        return tenant_required();
    };
    match tag::Entity::find()
        .filter(tag::Column::TenantId.eq(tenant_id))
        .all(db.get_ref())
        .await
    {
        Ok(tags) => HttpResponse::Ok().json(tags.into_iter().map(tag_response).collect::<Vec<_>>()),
        Err(error) => {
            tracing::error!(%error, "Failed to list tags");
            server_error("Failed to list tags")
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/tags",
    request_body = CreateTagRequest,
    responses((status = 200, description = "Tag created", body = TagResponse)),
    tag = "Contacts"
)]
#[post("/tags")]
pub async fn create_tag(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    body: web::Json<CreateTagRequest>,
) -> HttpResponse {
    let Some(tenant_id) = current.0.tenant_id else {
        return tenant_required();
    };
    match create_tag_model(db.get_ref(), tenant_id, &body.name, body.color.clone()).await {
        Ok(tag) => HttpResponse::Ok().json(tag_response(tag)),
        Err(response) => response,
    }
}

fn tag_response(tag: tag::Model) -> TagResponse {
    TagResponse {
        id: tag.id,
        tenant_id: tag.tenant_id,
        name: tag.name,
        color: tag.color,
    }
}

fn contact_to_response(contact: contact::Model, tags: Vec<TagResponse>) -> ContactResponse {
    ContactResponse {
        id: contact.id,
        tenant_id: contact.tenant_id,
        phone: contact.phone,
        name: contact.name,
        email: contact.email,
        company: contact.company,
        notes: contact.notes,
        owner_user_id: contact.owner_user_id,
        tags,
    }
}

async fn contact_responses(
    db: &DatabaseConnection,
    contacts: Vec<contact::Model>,
) -> Result<Vec<ContactResponse>, HttpResponse> {
    let mut responses = Vec::with_capacity(contacts.len());
    for contact in contacts {
        responses.push(contact_response(db, contact).await?);
    }
    Ok(responses)
}

async fn contact_response(
    db: &DatabaseConnection,
    contact: contact::Model,
) -> Result<ContactResponse, HttpResponse> {
    let tags = tags_for_contact(db, contact.tenant_id, contact.id).await?;
    Ok(contact_to_response(contact, tags))
}

async fn tags_for_contact(
    db: &DatabaseConnection,
    tenant_id: i32,
    contact_id: i32,
) -> Result<Vec<TagResponse>, HttpResponse> {
    let links = contact_tag::Entity::find()
        .filter(contact_tag::Column::ContactId.eq(contact_id))
        .all(db)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Failed to load contact tags");
            server_error("Failed to load contact tags")
        })?;
    let tag_ids = links
        .into_iter()
        .map(|link| link.tag_id)
        .collect::<Vec<_>>();
    if tag_ids.is_empty() {
        return Ok(vec![]);
    }
    let tags = tag::Entity::find()
        .filter(tag::Column::TenantId.eq(tenant_id))
        .filter(tag::Column::Id.is_in(tag_ids))
        .all(db)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Failed to load tags");
            server_error("Failed to load tags")
        })?;
    Ok(tags.into_iter().map(tag_response).collect())
}

async fn contact_ids_for_tag(
    db: &DatabaseConnection,
    tenant_id: i32,
    tag_name: &str,
) -> Result<Vec<i32>, HttpResponse> {
    let tag = tag::Entity::find()
        .filter(tag::Column::TenantId.eq(tenant_id))
        .filter(tag::Column::Name.eq(tag_name))
        .one(db)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Failed to load tag filter");
            server_error("Failed to list contacts")
        })?;
    let Some(tag) = tag else {
        return Ok(vec![]);
    };
    let links = contact_tag::Entity::find()
        .filter(contact_tag::Column::TagId.eq(tag.id))
        .all(db)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Failed to load tag contacts");
            server_error("Failed to list contacts")
        })?;
    Ok(links.into_iter().map(|link| link.contact_id).collect())
}

async fn load_contact(
    db: &DatabaseConnection,
    tenant_id: i32,
    contact_id: i32,
) -> Result<Option<contact::Model>, HttpResponse> {
    contact::Entity::find()
        .filter(contact::Column::TenantId.eq(tenant_id))
        .filter(contact::Column::Id.eq(contact_id))
        .one(db)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Failed to load contact");
            server_error("Failed to load contact")
        })
}

async fn contact_exists(db: &DatabaseConnection, tenant_id: i32, contact_id: i32) -> bool {
    matches!(load_contact(db, tenant_id, contact_id).await, Ok(Some(_)))
}

async fn user_belongs_to_tenant(db: &DatabaseConnection, tenant_id: i32, user_id: i32) -> bool {
    matches!(
        user::Entity::find()
            .filter(user::Column::Id.eq(user_id))
            .filter(user::Column::TenantId.eq(tenant_id))
            .one(db)
            .await,
        Ok(Some(_))
    )
}

async fn tag_exists(db: &DatabaseConnection, tenant_id: i32, tag_id: i32) -> bool {
    matches!(
        tag::Entity::find()
            .filter(tag::Column::TenantId.eq(tenant_id))
            .filter(tag::Column::Id.eq(tag_id))
            .one(db)
            .await,
        Ok(Some(_))
    )
}

async fn resolve_or_create_tag(
    db: &DatabaseConnection,
    tenant_id: i32,
    body: &AttachTagRequest,
) -> Result<tag::Model, HttpResponse> {
    if let Some(tag_id) = body.tag_id {
        return tag::Entity::find()
            .filter(tag::Column::TenantId.eq(tenant_id))
            .filter(tag::Column::Id.eq(tag_id))
            .one(db)
            .await
            .map_err(|error| {
                tracing::error!(%error, "Failed to load tag");
                server_error("Failed to load tag")
            })?
            .ok_or_else(|| not_found("Tag not found"));
    }
    let Some(name) = body.name.as_ref().filter(|name| !name.trim().is_empty()) else {
        return Err(HttpResponse::BadRequest()
            .json(serde_json::json!({"success": false, "error": "tag_id or name required"})));
    };
    if let Some(existing) = tag::Entity::find()
        .filter(tag::Column::TenantId.eq(tenant_id))
        .filter(tag::Column::Name.eq(name.trim()))
        .one(db)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Failed to load tag");
            server_error("Failed to load tag")
        })?
    {
        return Ok(existing);
    }
    create_tag_model(db, tenant_id, name.trim(), body.color.clone()).await
}

async fn create_tag_model(
    db: &DatabaseConnection,
    tenant_id: i32,
    name: &str,
    color: Option<String>,
) -> Result<tag::Model, HttpResponse> {
    let name = name.trim();
    if name.is_empty() {
        return Err(HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": "Tag name required"
        })));
    }

    tag::ActiveModel {
        tenant_id: Set(tenant_id),
        name: Set(name.to_string()),
        color: Set(color),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|error| {
        tracing::error!(%error, "Failed to create tag");
        HttpResponse::Conflict()
            .json(serde_json::json!({"success": false, "error": "Failed to create tag"}))
    })
}

fn tenant_required() -> HttpResponse {
    HttpResponse::Forbidden()
        .json(serde_json::json!({"success": false, "error": "Tenant context required"}))
}

fn not_found(message: &str) -> HttpResponse {
    HttpResponse::NotFound().json(serde_json::json!({"success": false, "error": message}))
}

fn server_error(message: &str) -> HttpResponse {
    HttpResponse::InternalServerError()
        .json(serde_json::json!({"success": false, "error": message}))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_contacts)
        .service(get_contact)
        .service(update_contact)
        .service(attach_tag)
        .service(detach_tag)
        .service(list_tags)
        .service(create_tag);
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test as awtest, web, App};
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, Database};

    use crate::auth::jwt::{build_claims, encode_jwt};
    use crate::auth::password::hash_password;
    use crate::config::AppConfig;
    use crate::models::{contact, tenant, user};

    #[actix_rt::test]
    async fn contacts_and_tags_are_tenant_scoped() {
        let _ = dotenvy::dotenv();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL for contacts tests");
        let setup_db = Database::connect(&database_url).await.expect("db connect");
        let suffix = chrono::Utc::now().timestamp_nanos_opt().unwrap();

        let tenant = tenant::ActiveModel {
            name: Set("Contacts Test Tenant".to_string()),
            slug: Set(format!("contacts-test-{suffix}")),
            is_active: Set(true),
            ..Default::default()
        }
        .insert(&setup_db)
        .await
        .expect("tenant");
        let other_tenant = tenant::ActiveModel {
            name: Set("Other Contacts Tenant".to_string()),
            slug: Set(format!("contacts-other-{suffix}")),
            is_active: Set(true),
            ..Default::default()
        }
        .insert(&setup_db)
        .await
        .expect("other tenant");
        let user = user::ActiveModel {
            email: Set(format!("contacts-user-{suffix}@example.com")),
            name: Set("Contacts User".to_string()),
            password_hash: Set(hash_password("contacts123456").expect("hash")),
            tenant_id: Set(Some(tenant.id)),
            is_superadmin: Set(false),
            is_active: Set(true),
            ..Default::default()
        }
        .insert(&setup_db)
        .await
        .expect("user");
        let contact = contact::ActiveModel {
            tenant_id: Set(tenant.id),
            phone: Set(format!("628{suffix}")),
            name: Set(Some("Alice Contact".to_string())),
            email: Set(Some("alice@example.com".to_string())),
            company: Set(Some("Acme".to_string())),
            notes: Set(Some("vip note".to_string())),
            owner_user_id: Set(Some(user.id)),
            ..Default::default()
        }
        .insert(&setup_db)
        .await
        .expect("contact");
        let other_user = user::ActiveModel {
            email: Set(format!("other-contacts-user-{suffix}@example.com")),
            name: Set("Other Contacts User".to_string()),
            password_hash: Set(hash_password("contacts123456").expect("hash")),
            tenant_id: Set(Some(other_tenant.id)),
            is_superadmin: Set(false),
            is_active: Set(true),
            ..Default::default()
        }
        .insert(&setup_db)
        .await
        .expect("other user");
        contact::ActiveModel {
            tenant_id: Set(other_tenant.id),
            phone: Set(format!("629{suffix}")),
            name: Set(Some("Other Contact".to_string())),
            ..Default::default()
        }
        .insert(&setup_db)
        .await
        .expect("other contact");

        let token = encode_jwt(
            &build_claims(user.id, Some(tenant.id), false, 3600),
            "test-contacts-secret",
        )
        .expect("token");
        let app_db = Database::connect(&database_url).await.expect("db connect");
        let config = AppConfig {
            database_url,
            jwt_secret: "test-contacts-secret".to_string(),
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            app_base_url: "http://localhost:8080".into(),
            storage_backend: "local".to_string(),
            storage_local_dir: "media".to_string(),
            r2_endpoint: None,
            r2_access_key_id: None,
            r2_secret_access_key: None,
            r2_bucket: None,
            r2_public_base_url: None,
        };
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(app_db))
                .app_data(web::Data::new(config))
                .configure(super::configure),
        )
        .await;

        let list_req = awtest::TestRequest::get()
            .uri("/contacts")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let list_resp = awtest::call_service(&app, list_req).await;
        assert_eq!(list_resp.status(), StatusCode::OK);
        let list_body: serde_json::Value =
            serde_json::from_slice(&awtest::read_body(list_resp).await).unwrap();
        assert_eq!(list_body["total"], 1);
        assert_eq!(list_body["data"][0]["tenant_id"], tenant.id);

        let patch_req = awtest::TestRequest::patch()
            .uri(&format!("/contacts/{}", contact.id))
            .insert_header(("Authorization", format!("Bearer {token}")))
            .set_json(serde_json::json!({"company": "Updated Co"}))
            .to_request();
        let patch_resp = awtest::call_service(&app, patch_req).await;
        assert_eq!(patch_resp.status(), StatusCode::OK);
        let patch_body: serde_json::Value =
            serde_json::from_slice(&awtest::read_body(patch_resp).await).unwrap();
        assert_eq!(patch_body["company"], "Updated Co");

        let cross_owner_req = awtest::TestRequest::patch()
            .uri(&format!("/contacts/{}", contact.id))
            .insert_header(("Authorization", format!("Bearer {token}")))
            .set_json(serde_json::json!({"owner_user_id": other_user.id}))
            .to_request();
        let cross_owner_resp = awtest::call_service(&app, cross_owner_req).await;
        assert_eq!(cross_owner_resp.status(), StatusCode::BAD_REQUEST);

        let attach_req = awtest::TestRequest::post()
            .uri(&format!("/contacts/{}/tags", contact.id))
            .insert_header(("Authorization", format!("Bearer {token}")))
            .set_json(serde_json::json!({"name": "vip", "color": "#16a34a"}))
            .to_request();
        let attach_resp = awtest::call_service(&app, attach_req).await;
        assert_eq!(attach_resp.status(), StatusCode::OK);
        let attach_body: serde_json::Value =
            serde_json::from_slice(&awtest::read_body(attach_resp).await).unwrap();
        let tag_id = attach_body["tags"][0]["id"].as_i64().unwrap();
        assert_eq!(attach_body["tags"][0]["name"], "vip");

        let filtered_req = awtest::TestRequest::get()
            .uri("/contacts?tag=vip")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let filtered_resp = awtest::call_service(&app, filtered_req).await;
        assert_eq!(filtered_resp.status(), StatusCode::OK);
        let filtered_body: serde_json::Value =
            serde_json::from_slice(&awtest::read_body(filtered_resp).await).unwrap();
        assert_eq!(filtered_body["total"], 1);

        let detach_req = awtest::TestRequest::delete()
            .uri(&format!("/contacts/{}/tags/{tag_id}", contact.id))
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let detach_resp = awtest::call_service(&app, detach_req).await;
        assert_eq!(detach_resp.status(), StatusCode::OK);
    }
}
