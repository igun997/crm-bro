use actix_web::{delete, get, patch, post, web, HttpResponse};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::password::hash_password;
use crate::auth::CurrentUser;
use crate::domain::tenants::{SeaOrmTenantRepository, TenantService};
use crate::models::{permission, role, role_permission, tenant, user, user_role};
use crate::rbac::{default_tenant_roles, permissions as permission_codes};

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"name": "Acme Corp", "slug": "acme"}))]
pub struct CreateTenantRequest {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"id": 1, "name": "Acme Corp", "slug": "acme", "is_active": true}))]
pub struct TenantResponse {
    pub id: i32,
    pub name: String,
    pub slug: String,
    pub is_active: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"email": "agent@acme.com", "password": "s3cret", "name": "John Agent"}))]
pub struct CreateTenantUserRequest {
    pub email: String,
    pub password: String,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"id": 2, "email": "agent@acme.com", "name": "John Agent", "tenant_id": 1, "is_superadmin": false, "is_active": true}))]
pub struct AdminUserResponse {
    pub id: i32,
    pub email: String,
    pub name: String,
    pub tenant_id: Option<i32>,
    pub is_superadmin: bool,
    pub is_active: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"name": "Jane Agent", "is_active": true}))]
pub struct PatchUserRequest {
    pub email: Option<String>,
    pub name: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"password": "newP@ss123"}))]
pub struct ResetPasswordRequest {
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"role_id": 1}))]
pub struct AssignRoleRequest {
    pub role_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"id": 1, "tenant_id": 1, "name": "Tenant Admin", "description": "Full tenant access", "is_system": true, "permissions": ["tenant:manage", "users:manage", "settings:wa:manage"]}))]
pub struct RoleResponse {
    pub id: i32,
    pub tenant_id: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"name": "Custom Agent", "description": "Limited agent role", "permissions": ["chat:read", "chat:send", "contacts:read"]}))]
pub struct CreateRoleRequest {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"description": "Updated description", "permissions": ["chat:read", "chat:send"]}))]
pub struct PatchRoleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"id": 1, "code": "chat:read", "description": "View conversations and messages"}))]
pub struct PermissionResponse {
    pub id: i32,
    pub code: String,
    pub description: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/admin/tenants",
    request_body = CreateTenantRequest,
    responses(
        (status = 200, description = "Tenant created", body = TenantResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    tag = "Admin"
)]
#[post("/admin/tenants")]
pub async fn create_tenant(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    body: web::Json<CreateTenantRequest>,
) -> HttpResponse {
    if !current.0.is_superadmin {
        return forbidden();
    }

    let service = TenantService::new(SeaOrmTenantRepository::new(db.get_ref().clone()));

    match service
        .create_tenant(body.name.clone(), body.slug.clone())
        .await
    {
        Ok(tenant) => {
            if let Err(error) = seed_default_tenant_roles(db.get_ref(), tenant.id()).await {
                tracing::warn!(%error, tenant_id = tenant.id(), "Failed to seed default tenant roles");
            }
            HttpResponse::Ok().json(TenantResponse {
                id: tenant.id(),
                name: tenant.name().to_string(),
                slug: tenant.slug().to_string(),
                is_active: tenant.is_active(),
            })
        }
        Err(error) => {
            tracing::error!(%error, "Failed to create tenant");
            HttpResponse::Conflict().json(serde_json::json!({
                "success": false,
                "error": "Tenant creation failed"
            }))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/tenants/{tenant_id}/users",
    request_body = CreateTenantUserRequest,
    params(("tenant_id" = i32, Path, description = "Tenant id")),
    responses(
        (status = 200, description = "User created", body = AdminUserResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    tag = "Admin"
)]
#[post("/admin/tenants/{tenant_id}/users")]
pub async fn create_tenant_user(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
    body: web::Json<CreateTenantUserRequest>,
) -> HttpResponse {
    let tenant_id = path.into_inner();
    if let Err(resp) = can_manage_tenant(&current.0, tenant_id) {
        return resp;
    }

    let tenant_exists = match tenant::Entity::find_by_id(tenant_id)
        .one(db.get_ref())
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(error) => {
            tracing::error!(%error, "Failed to load tenant");
            return server_error("Tenant lookup failed");
        }
    };

    if !tenant_exists {
        return HttpResponse::NotFound().json(serde_json::json!({
            "success": false,
            "error": "Tenant not found"
        }));
    }

    let password_hash = match hash_password(&body.password) {
        Ok(hash) => hash,
        Err(error) => {
            tracing::error!(%error, "Failed to hash password");
            return server_error("User creation failed");
        }
    };

    let new_user = user::ActiveModel {
        email: Set(body.email.clone()),
        name: Set(body.name.clone()),
        password_hash: Set(password_hash),
        tenant_id: Set(Some(tenant_id)),
        is_superadmin: Set(false),
        is_active: Set(true),
        ..Default::default()
    };

    let created_user = match new_user.insert(db.get_ref()).await {
        Ok(user) => user,
        Err(error) => {
            tracing::error!(%error, "Failed to create tenant user");
            return HttpResponse::Conflict().json(serde_json::json!({
                "success": false,
                "error": "User creation failed"
            }));
        }
    };

    if let Err(error) = attach_default_role(db.get_ref(), tenant_id, created_user.id).await {
        tracing::warn!(%error, "Failed to attach default tenant role");
    }

    HttpResponse::Ok().json(AdminUserResponse {
        id: created_user.id,
        email: created_user.email,
        name: created_user.name,
        tenant_id: created_user.tenant_id,
        is_superadmin: created_user.is_superadmin,
        is_active: created_user.is_active,
    })
}

fn user_response(user: user::Model) -> AdminUserResponse {
    AdminUserResponse {
        id: user.id,
        email: user.email,
        name: user.name,
        tenant_id: user.tenant_id,
        is_superadmin: user.is_superadmin,
        is_active: user.is_active,
    }
}

fn can_manage_tenant(
    ctx: &crate::auth::context::AuthContext,
    tenant_id: i32,
) -> Result<(), HttpResponse> {
    if ctx.is_superadmin {
        return Ok(());
    }
    if ctx.tenant_id == Some(tenant_id)
        && ctx
            .permissions
            .contains(permission_codes::ADMIN_USERS_MANAGE)
    {
        return Ok(());
    }
    Err(forbidden())
}

async fn can_manage_user(
    db: &DatabaseConnection,
    ctx: &crate::auth::context::AuthContext,
    user_id: i32,
) -> Result<user::Model, HttpResponse> {
    let Some(user) = user::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Failed to load user");
            server_error("User lookup failed")
        })?
    else {
        return Err(HttpResponse::NotFound().json(serde_json::json!({
            "success": false,
            "error": "User not found"
        })));
    };

    let Some(tenant_id) = user.tenant_id else {
        return Err(forbidden());
    };
    can_manage_tenant(ctx, tenant_id)?;
    Ok(user)
}

async fn role_response(
    db: &DatabaseConnection,
    role: role::Model,
) -> Result<RoleResponse, sea_orm::DbErr> {
    let role_permissions = role_permission::Entity::find()
        .filter(role_permission::Column::RoleId.eq(role.id))
        .all(db)
        .await?;
    let permission_ids = role_permissions
        .into_iter()
        .map(|rp| rp.permission_id)
        .collect::<Vec<_>>();
    let permissions = if permission_ids.is_empty() {
        Vec::new()
    } else {
        permission::Entity::find()
            .filter(permission::Column::Id.is_in(permission_ids))
            .order_by_asc(permission::Column::Code)
            .all(db)
            .await?
            .into_iter()
            .map(|permission| permission.code)
            .collect()
    };

    Ok(RoleResponse {
        id: role.id,
        tenant_id: role.tenant_id,
        name: role.name,
        description: role.description,
        is_system: role.is_system,
        permissions,
    })
}

async fn set_role_permissions(
    db: &DatabaseConnection,
    role_id: i32,
    codes: &[String],
) -> Result<(), String> {
    role_permission::Entity::delete_many()
        .filter(role_permission::Column::RoleId.eq(role_id))
        .exec(db)
        .await
        .map_err(|error| format!("Delete role permissions failed: {error}"))?;

    if codes.is_empty() {
        return Ok(());
    }

    let permissions = permission::Entity::find()
        .filter(permission::Column::Code.is_in(codes.iter().cloned()))
        .all(db)
        .await
        .map_err(|error| format!("Load permissions failed: {error}"))?;

    if permissions.len() != codes.len() {
        return Err("Unknown permission code".to_string());
    }

    for permission in permissions {
        role_permission::ActiveModel {
            role_id: Set(role_id),
            permission_id: Set(permission.id),
        }
        .insert(db)
        .await
        .map_err(|error| format!("Insert role permission failed: {error}"))?;
    }

    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/admin/tenants/{tenant_id}/users",
    params(("tenant_id" = i32, Path, description = "Tenant id")),
    responses((status = 200, description = "Tenant users", body = Vec<AdminUserResponse>)),
    tag = "Admin"
)]
#[get("/admin/tenants/{tenant_id}/users")]
pub async fn list_tenant_users(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
) -> HttpResponse {
    let tenant_id = path.into_inner();
    if let Err(resp) = can_manage_tenant(&current.0, tenant_id) {
        return resp;
    }

    match user::Entity::find()
        .filter(user::Column::TenantId.eq(tenant_id))
        .order_by_asc(user::Column::Id)
        .all(db.get_ref())
        .await
    {
        Ok(users) => {
            HttpResponse::Ok().json(users.into_iter().map(user_response).collect::<Vec<_>>())
        }
        Err(error) => {
            tracing::error!(%error, "Failed to list tenant users");
            server_error("User list failed")
        }
    }
}

#[utoipa::path(get, path = "/api/admin/users/{user_id}", params(("user_id" = i32, Path, description = "User id")), responses((status = 200, description = "User", body = AdminUserResponse)), tag = "Admin")]
#[get("/admin/users/{user_id}")]
pub async fn get_user(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
) -> HttpResponse {
    match can_manage_user(db.get_ref(), &current.0, path.into_inner()).await {
        Ok(user) => HttpResponse::Ok().json(user_response(user)),
        Err(resp) => resp,
    }
}

#[utoipa::path(patch, path = "/api/admin/users/{user_id}", request_body = PatchUserRequest, params(("user_id" = i32, Path, description = "User id")), responses((status = 200, description = "User updated", body = AdminUserResponse)), tag = "Admin")]
#[patch("/admin/users/{user_id}")]
pub async fn update_user(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
    body: web::Json<PatchUserRequest>,
) -> HttpResponse {
    let user = match can_manage_user(db.get_ref(), &current.0, path.into_inner()).await {
        Ok(user) => user,
        Err(resp) => return resp,
    };

    let mut update: user::ActiveModel = user.into();
    if let Some(email) = &body.email {
        update.email = Set(email.clone());
    }
    if let Some(name) = &body.name {
        update.name = Set(name.clone());
    }
    if let Some(is_active) = body.is_active {
        update.is_active = Set(is_active);
    }

    match update.update(db.get_ref()).await {
        Ok(user) => HttpResponse::Ok().json(user_response(user)),
        Err(error) => {
            tracing::error!(%error, "Failed to update user");
            HttpResponse::Conflict()
                .json(serde_json::json!({"success": false, "error": "User update failed"}))
        }
    }
}

#[utoipa::path(post, path = "/api/admin/users/{user_id}/reset-password", request_body = ResetPasswordRequest, params(("user_id" = i32, Path, description = "User id")), responses((status = 200, description = "Password reset")), tag = "Admin")]
#[post("/admin/users/{user_id}/reset-password")]
pub async fn reset_user_password(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
    body: web::Json<ResetPasswordRequest>,
) -> HttpResponse {
    let user = match can_manage_user(db.get_ref(), &current.0, path.into_inner()).await {
        Ok(user) => user,
        Err(resp) => return resp,
    };
    let password_hash = match hash_password(&body.password) {
        Ok(hash) => hash,
        Err(error) => {
            tracing::error!(%error, "Failed to hash password");
            return server_error("Password reset failed");
        }
    };
    let mut update: user::ActiveModel = user.into();
    update.password_hash = Set(password_hash);
    match update.update(db.get_ref()).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"success": true})),
        Err(error) => {
            tracing::error!(%error, "Failed to reset password");
            server_error("Password reset failed")
        }
    }
}

#[utoipa::path(post, path = "/api/admin/users/{user_id}/roles", request_body = AssignRoleRequest, params(("user_id" = i32, Path, description = "User id")), responses((status = 200, description = "Role assigned")), tag = "Admin")]
#[post("/admin/users/{user_id}/roles")]
pub async fn assign_user_role(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
    body: web::Json<AssignRoleRequest>,
) -> HttpResponse {
    let user = match can_manage_user(db.get_ref(), &current.0, path.into_inner()).await {
        Ok(user) => user,
        Err(resp) => return resp,
    };
    let role = match role::Entity::find_by_id(body.role_id)
        .one(db.get_ref())
        .await
    {
        Ok(Some(role)) => role,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(serde_json::json!({"success": false, "error": "Role not found"}))
        }
        Err(error) => {
            tracing::error!(%error, "Failed to load role");
            return server_error("Role lookup failed");
        }
    };
    if role.tenant_id != user.tenant_id {
        return forbidden();
    }
    let existing = user_role::Entity::find()
        .filter(user_role::Column::UserId.eq(user.id))
        .filter(user_role::Column::RoleId.eq(role.id))
        .one(db.get_ref())
        .await;
    match existing {
        Ok(Some(_)) => HttpResponse::Ok().json(serde_json::json!({"success": true})),
        Ok(None) => match (user_role::ActiveModel {
            user_id: Set(user.id),
            role_id: Set(role.id),
        })
        .insert(db.get_ref())
        .await
        {
            Ok(_) => HttpResponse::Ok().json(serde_json::json!({"success": true})),
            Err(error) => {
                tracing::error!(%error, "Failed to assign role");
                server_error("Role assignment failed")
            }
        },
        Err(error) => {
            tracing::error!(%error, "Failed to check user role");
            server_error("Role assignment failed")
        }
    }
}

#[utoipa::path(delete, path = "/api/admin/users/{user_id}/roles/{role_id}", params(("user_id" = i32, Path, description = "User id"), ("role_id" = i32, Path, description = "Role id")), responses((status = 200, description = "Role removed")), tag = "Admin")]
#[delete("/admin/users/{user_id}/roles/{role_id}")]
pub async fn remove_user_role(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<(i32, i32)>,
) -> HttpResponse {
    let (user_id, role_id) = path.into_inner();
    if let Err(resp) = can_manage_user(db.get_ref(), &current.0, user_id).await {
        return resp;
    }
    match user_role::Entity::delete_many()
        .filter(user_role::Column::UserId.eq(user_id))
        .filter(user_role::Column::RoleId.eq(role_id))
        .exec(db.get_ref())
        .await
    {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"success": true})),
        Err(error) => {
            tracing::error!(%error, "Failed to remove role");
            server_error("Role removal failed")
        }
    }
}

#[utoipa::path(get, path = "/api/admin/tenants/{tenant_id}/roles", params(("tenant_id" = i32, Path, description = "Tenant id")), responses((status = 200, description = "Tenant roles", body = Vec<RoleResponse>)), tag = "Admin")]
#[get("/admin/tenants/{tenant_id}/roles")]
pub async fn list_tenant_roles(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
) -> HttpResponse {
    let tenant_id = path.into_inner();
    if let Err(resp) = can_manage_tenant(&current.0, tenant_id) {
        return resp;
    }
    match role::Entity::find()
        .filter(role::Column::TenantId.eq(tenant_id))
        .order_by_asc(role::Column::Id)
        .all(db.get_ref())
        .await
    {
        Ok(roles) => {
            let mut responses = Vec::new();
            for role in roles {
                match role_response(db.get_ref(), role).await {
                    Ok(resp) => responses.push(resp),
                    Err(error) => {
                        tracing::error!(%error, "Failed to load role permissions");
                        return server_error("Role list failed");
                    }
                }
            }
            HttpResponse::Ok().json(responses)
        }
        Err(error) => {
            tracing::error!(%error, "Failed to list roles");
            server_error("Role list failed")
        }
    }
}

#[utoipa::path(post, path = "/api/admin/tenants/{tenant_id}/roles", request_body = CreateRoleRequest, params(("tenant_id" = i32, Path, description = "Tenant id")), responses((status = 200, description = "Role created", body = RoleResponse)), tag = "Admin")]
#[post("/admin/tenants/{tenant_id}/roles")]
pub async fn create_tenant_role(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
    body: web::Json<CreateRoleRequest>,
) -> HttpResponse {
    let tenant_id = path.into_inner();
    if let Err(resp) = can_manage_tenant(&current.0, tenant_id) {
        return resp;
    }
    let new_role = role::ActiveModel {
        tenant_id: Set(Some(tenant_id)),
        name: Set(body.name.clone()),
        description: Set(body.description.clone()),
        is_system: Set(false),
        ..Default::default()
    };
    let role = match new_role.insert(db.get_ref()).await {
        Ok(role) => role,
        Err(error) => {
            tracing::error!(%error, "Failed to create role");
            return HttpResponse::Conflict()
                .json(serde_json::json!({"success": false, "error": "Role creation failed"}));
        }
    };
    if let Err(error) = set_role_permissions(db.get_ref(), role.id, &body.permissions).await {
        tracing::error!(%error, "Failed to set role permissions");
        return HttpResponse::BadRequest()
            .json(serde_json::json!({"success": false, "error": error}));
    }
    match role_response(db.get_ref(), role).await {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(error) => {
            tracing::error!(%error, "Failed to load role response");
            server_error("Role creation failed")
        }
    }
}

#[utoipa::path(patch, path = "/api/admin/roles/{role_id}", request_body = PatchRoleRequest, params(("role_id" = i32, Path, description = "Role id")), responses((status = 200, description = "Role updated", body = RoleResponse)), tag = "Admin")]
#[patch("/admin/roles/{role_id}")]
pub async fn update_role(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    path: web::Path<i32>,
    body: web::Json<PatchRoleRequest>,
) -> HttpResponse {
    let role = match role::Entity::find_by_id(path.into_inner())
        .one(db.get_ref())
        .await
    {
        Ok(Some(role)) => role,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(serde_json::json!({"success": false, "error": "Role not found"}))
        }
        Err(error) => {
            tracing::error!(%error, "Failed to load role");
            return server_error("Role lookup failed");
        }
    };
    let Some(tenant_id) = role.tenant_id else {
        return forbidden();
    };
    if let Err(resp) = can_manage_tenant(&current.0, tenant_id) {
        return resp;
    }
    if role.is_system {
        return HttpResponse::Conflict()
            .json(serde_json::json!({"success": false, "error": "System role cannot be updated"}));
    }
    let mut update: role::ActiveModel = role.clone().into();
    if let Some(name) = &body.name {
        update.name = Set(name.clone());
    }
    if let Some(description) = &body.description {
        update.description = Set(Some(description.clone()));
    }
    let role = match update.update(db.get_ref()).await {
        Ok(role) => role,
        Err(error) => {
            tracing::error!(%error, "Failed to update role");
            return HttpResponse::Conflict()
                .json(serde_json::json!({"success": false, "error": "Role update failed"}));
        }
    };
    if let Some(permissions) = &body.permissions {
        if let Err(error) = set_role_permissions(db.get_ref(), role.id, permissions).await {
            tracing::error!(%error, "Failed to set role permissions");
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"success": false, "error": error}));
        }
    }
    match role_response(db.get_ref(), role).await {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(error) => {
            tracing::error!(%error, "Failed to load role response");
            server_error("Role update failed")
        }
    }
}

#[utoipa::path(get, path = "/api/admin/permissions", responses((status = 200, description = "Permissions", body = Vec<PermissionResponse>)), tag = "Admin")]
#[get("/admin/permissions")]
pub async fn list_permissions(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
) -> HttpResponse {
    if !current.0.is_superadmin
        && !current
            .0
            .permissions
            .contains(permission_codes::ADMIN_USERS_MANAGE)
    {
        return forbidden();
    }
    match permission::Entity::find()
        .order_by_asc(permission::Column::Code)
        .all(db.get_ref())
        .await
    {
        Ok(permissions) => HttpResponse::Ok().json(
            permissions
                .into_iter()
                .map(|permission| PermissionResponse {
                    id: permission.id,
                    code: permission.code,
                    description: permission.description,
                })
                .collect::<Vec<_>>(),
        ),
        Err(error) => {
            tracing::error!(%error, "Failed to list permissions");
            server_error("Permission list failed")
        }
    }
}

async fn seed_default_tenant_roles(db: &DatabaseConnection, tenant_id: i32) -> Result<(), String> {
    for role_def in default_tenant_roles() {
        let existing = role::Entity::find()
            .filter(role::Column::TenantId.eq(tenant_id))
            .filter(role::Column::Name.eq(role_def.name))
            .one(db)
            .await
            .map_err(|error| format!("Load role failed: {error}"))?;
        let role = match existing {
            Some(role) => role,
            None => role::ActiveModel {
                tenant_id: Set(Some(tenant_id)),
                name: Set(role_def.name.to_string()),
                description: Set(Some(role_def.description.to_string())),
                is_system: Set(true),
                ..Default::default()
            }
            .insert(db)
            .await
            .map_err(|error| format!("Create role failed: {error}"))?,
        };
        set_role_permissions(
            db,
            role.id,
            &role_def
                .permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect::<Vec<_>>(),
        )
        .await?;
    }
    Ok(())
}

async fn attach_default_role(
    db: &DatabaseConnection,
    tenant_id: i32,
    user_id: i32,
) -> Result<(), sea_orm::DbErr> {
    let Some(default_role) = role::Entity::find()
        .filter(role::Column::TenantId.eq(tenant_id))
        .filter(role::Column::Name.eq(crate::rbac::roles::AGENT))
        .order_by_asc(role::Column::Id)
        .one(db)
        .await?
    else {
        return Ok(());
    };

    let existing = user_role::Entity::find()
        .filter(user_role::Column::UserId.eq(user_id))
        .filter(user_role::Column::RoleId.eq(default_role.id))
        .one(db)
        .await?;

    if existing.is_none() {
        let link = user_role::ActiveModel {
            user_id: Set(user_id),
            role_id: Set(default_role.id),
        };
        link.insert(db).await?;
    }

    Ok(())
}

fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().json(serde_json::json!({
        "success": false,
        "error": "Forbidden"
    }))
}

fn server_error(error: &str) -> HttpResponse {
    HttpResponse::InternalServerError().json(serde_json::json!({
        "success": false,
        "error": error
    }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(create_tenant)
        .service(create_tenant_user)
        .service(list_tenant_users)
        .service(get_user)
        .service(update_user)
        .service(reset_user_password)
        .service(assign_user_role)
        .service(remove_user_role)
        .service(list_tenant_roles)
        .service(create_tenant_role)
        .service(update_role)
        .service(list_permissions);
}

#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test, web, App};
    use sea_orm::Database;

    use crate::{
        auth::jwt::{build_claims, encode_jwt},
        infrastructure::config::AppConfig,
    };

    async fn post_tenant(
        token: Option<String>,
        name: &str,
        slug: &str,
    ) -> (StatusCode, serde_json::Value) {
        let _ = dotenvy::dotenv();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL for admin tests");
        let db = Database::connect(&database_url).await.expect("db connect");
        let config = AppConfig {
            database_url,
            jwt_secret: "test-admin-secret".to_string(),
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

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(db))
                .app_data(web::Data::new(config))
                .configure(super::configure),
        )
        .await;

        let mut req = test::TestRequest::post()
            .uri("/admin/tenants")
            .set_json(serde_json::json!({"name": name, "slug": slug}));
        if let Some(token) = token {
            req = req.insert_header(("Authorization", format!("Bearer {token}")));
        }

        let resp = test::call_service(&app, req.to_request()).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        let json = serde_json::from_slice(&body).unwrap_or_else(|_| serde_json::json!({}));
        (status, json)
    }

    fn admin_token() -> String {
        token_for_user(1)
    }

    fn token_for_user(user_id: i32) -> String {
        let claims = build_claims(user_id, None, true, 3600);
        encode_jwt(&claims, "test-admin-secret").expect("token")
    }

    async fn admin_request(
        method: &str,
        uri: &str,
        token: Option<String>,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let _ = dotenvy::dotenv();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL for admin tests");
        let db = Database::connect(&database_url).await.expect("db connect");
        let config = AppConfig {
            database_url,
            jwt_secret: "test-admin-secret".to_string(),
            server_host: "127.0.0.1".to_string(),
            app_base_url: "http://localhost:8080".into(),
            server_port: 8080,
            storage_backend: "local".to_string(),
            storage_local_dir: "media".to_string(),
            r2_endpoint: None,
            r2_access_key_id: None,
            r2_secret_access_key: None,
            r2_bucket: None,
            r2_public_base_url: None,
        };

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(db))
                .app_data(web::Data::new(config))
                .configure(super::configure),
        )
        .await;

        let mut req = match method {
            "GET" => test::TestRequest::get().uri(uri),
            "PATCH" => test::TestRequest::patch().uri(uri),
            "POST" => test::TestRequest::post().uri(uri),
            "DELETE" => test::TestRequest::delete().uri(uri),
            _ => panic!("unsupported method"),
        };
        if let Some(body) = body {
            req = req.set_json(&body);
        }
        if let Some(token) = token {
            req = req.insert_header(("Authorization", format!("Bearer {token}")));
        }

        let resp = test::call_service(&app, req.to_request()).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        let json = serde_json::from_slice(&body).unwrap_or_else(|_| serde_json::json!({}));
        (status, json)
    }

    async fn post_tenant_user(
        token: Option<String>,
        tenant_id: i32,
        email: &str,
    ) -> (StatusCode, serde_json::Value) {
        let _ = dotenvy::dotenv();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL for admin tests");
        let db = Database::connect(&database_url).await.expect("db connect");
        let config = AppConfig {
            database_url,
            jwt_secret: "test-admin-secret".to_string(),
            app_base_url: "http://localhost:8080".into(),
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            storage_backend: "local".to_string(),
            storage_local_dir: "media".to_string(),
            r2_endpoint: None,
            r2_access_key_id: None,
            r2_secret_access_key: None,
            r2_bucket: None,
            r2_public_base_url: None,
        };

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(db))
                .app_data(web::Data::new(config))
                .configure(super::configure),
        )
        .await;

        let mut req = test::TestRequest::post()
            .uri(&format!("/admin/tenants/{tenant_id}/users"))
            .set_json(serde_json::json!({
                "email": email,
                "password": "agent123456",
                "name": "Agent Test"
            }));
        if let Some(token) = token {
            req = req.insert_header(("Authorization", format!("Bearer {token}")));
        }

        let resp = test::call_service(&app, req.to_request()).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        let json = serde_json::from_slice(&body).unwrap_or_else(|_| serde_json::json!({}));
        (status, json)
    }

    #[actix_rt::test]
    async fn superadmin_can_create_tenant() {
        let slug = format!(
            "test-tenant-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        );
        let (status, body) = post_tenant(Some(admin_token()), "Test Tenant", &slug).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["slug"], slug);
    }

    #[actix_rt::test]
    async fn missing_token_cannot_create_tenant() {
        let (status, _) = post_tenant(None, "No Auth", "no-auth").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[actix_rt::test]
    async fn superadmin_can_create_tenant_user() {
        let suffix = chrono::Utc::now().timestamp_nanos_opt().unwrap();
        let slug = format!("test-user-tenant-{suffix}");
        let (tenant_status, tenant_body) =
            post_tenant(Some(admin_token()), "User Tenant", &slug).await;
        assert_eq!(tenant_status, StatusCode::OK);
        let tenant_id = tenant_body["id"].as_i64().expect("tenant id") as i32;

        let email = format!("agent-{suffix}@example.com");
        let (user_status, user_body) =
            post_tenant_user(Some(admin_token()), tenant_id, &email).await;
        assert_eq!(user_status, StatusCode::OK);
        assert_eq!(user_body["email"], email);
        assert_eq!(user_body["tenant_id"], tenant_id);
        assert_eq!(user_body["is_superadmin"], false);
        assert_eq!(user_body["is_active"], true);
    }

    #[actix_rt::test]
    async fn tenant_user_cannot_create_tenant() {
        let suffix = chrono::Utc::now().timestamp_nanos_opt().unwrap();
        let slug = format!("test-forbidden-tenant-{suffix}");
        let (tenant_status, tenant_body) =
            post_tenant(Some(admin_token()), "Forbidden Tenant", &slug).await;
        assert_eq!(tenant_status, StatusCode::OK);
        let tenant_id = tenant_body["id"].as_i64().expect("tenant id") as i32;

        let email = format!("tenant-user-{suffix}@example.com");
        let (user_status, user_body) =
            post_tenant_user(Some(admin_token()), tenant_id, &email).await;
        assert_eq!(user_status, StatusCode::OK);
        let user_id = user_body["id"].as_i64().expect("user id") as i32;

        let denied_slug = format!("denied-{suffix}");
        let (denied_status, _) =
            post_tenant(Some(token_for_user(user_id)), "Denied", &denied_slug).await;
        assert_eq!(denied_status, StatusCode::FORBIDDEN);
    }

    #[actix_rt::test]
    async fn creating_tenant_seeds_default_roles() {
        let suffix = chrono::Utc::now().timestamp_nanos_opt().unwrap();
        let slug = format!("test-default-roles-{suffix}");
        let (tenant_status, tenant_body) =
            post_tenant(Some(admin_token()), "Default Roles Tenant", &slug).await;
        assert_eq!(tenant_status, StatusCode::OK);
        let tenant_id = tenant_body["id"].as_i64().expect("tenant id") as i32;

        let (roles_status, roles) = admin_request(
            "GET",
            &format!("/admin/tenants/{tenant_id}/roles"),
            Some(admin_token()),
            None,
        )
        .await;
        assert_eq!(roles_status, StatusCode::OK);
        let names = roles
            .as_array()
            .unwrap()
            .iter()
            .map(|role| role["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(names.contains(&"tenant_admin"));
        assert!(names.contains(&"agent"));
        assert!(names.contains(&"viewer"));
    }

    #[actix_rt::test]
    async fn superadmin_can_manage_tenant_users_and_roles() {
        let suffix = chrono::Utc::now().timestamp_nanos_opt().unwrap();
        let slug = format!("test-admin-users-{suffix}");
        let (tenant_status, tenant_body) =
            post_tenant(Some(admin_token()), "Admin Users Tenant", &slug).await;
        assert_eq!(tenant_status, StatusCode::OK);
        let tenant_id = tenant_body["id"].as_i64().expect("tenant id") as i32;

        let email = format!("managed-{suffix}@example.com");
        let (user_status, user_body) =
            post_tenant_user(Some(admin_token()), tenant_id, &email).await;
        assert_eq!(user_status, StatusCode::OK);
        let user_id = user_body["id"].as_i64().expect("user id") as i32;

        let (list_status, users) = admin_request(
            "GET",
            &format!("/admin/tenants/{tenant_id}/users"),
            Some(admin_token()),
            None,
        )
        .await;
        assert_eq!(list_status, StatusCode::OK);
        assert!(users
            .as_array()
            .unwrap()
            .iter()
            .any(|user| user["id"] == user_id));

        let (patch_status, patched) = admin_request(
            "PATCH",
            &format!("/admin/users/{user_id}"),
            Some(admin_token()),
            Some(serde_json::json!({"name": "Managed Updated", "is_active": false})),
        )
        .await;
        assert_eq!(patch_status, StatusCode::OK);
        assert_eq!(patched["name"], "Managed Updated");
        assert_eq!(patched["is_active"], false);

        let (reset_status, reset_body) = admin_request(
            "POST",
            &format!("/admin/users/{user_id}/reset-password"),
            Some(admin_token()),
            Some(serde_json::json!({"password": "new-pass-123456"})),
        )
        .await;
        assert_eq!(reset_status, StatusCode::OK);
        assert_eq!(reset_body["success"], true);

        let (role_status, role_body) = admin_request(
            "POST",
            &format!("/admin/tenants/{tenant_id}/roles"),
            Some(admin_token()),
            Some(serde_json::json!({
                "name": format!("manager-{suffix}"),
                "description": "Can manage users",
                "permissions": ["admin.users.manage"]
            })),
        )
        .await;
        assert_eq!(role_status, StatusCode::OK);
        let role_id = role_body["id"].as_i64().expect("role id") as i32;
        assert!(role_body["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "admin.users.manage"));

        let (assign_status, assign_body) = admin_request(
            "POST",
            &format!("/admin/users/{user_id}/roles"),
            Some(admin_token()),
            Some(serde_json::json!({"role_id": role_id})),
        )
        .await;
        assert_eq!(assign_status, StatusCode::OK);
        assert_eq!(assign_body["success"], true);

        let (roles_status, roles) = admin_request(
            "GET",
            &format!("/admin/tenants/{tenant_id}/roles"),
            Some(admin_token()),
            None,
        )
        .await;
        assert_eq!(roles_status, StatusCode::OK);
        assert!(roles
            .as_array()
            .unwrap()
            .iter()
            .any(|role| role["id"] == role_id));

        let (permissions_status, permissions) =
            admin_request("GET", "/admin/permissions", Some(admin_token()), None).await;
        assert_eq!(permissions_status, StatusCode::OK);
        assert!(permissions
            .as_array()
            .unwrap()
            .iter()
            .any(|permission| permission["code"] == "admin.users.manage"));
    }

    #[actix_rt::test]
    async fn tenant_admin_can_manage_own_tenant_only() {
        let suffix = chrono::Utc::now().timestamp_nanos_opt().unwrap();
        let slug = format!("test-tenant-admin-{suffix}");
        let (tenant_status, tenant_body) =
            post_tenant(Some(admin_token()), "Tenant Admin Tenant", &slug).await;
        assert_eq!(tenant_status, StatusCode::OK);
        let tenant_id = tenant_body["id"].as_i64().expect("tenant id") as i32;

        let admin_email = format!("tenant-admin-{suffix}@example.com");
        let (admin_status, admin_body) =
            post_tenant_user(Some(admin_token()), tenant_id, &admin_email).await;
        assert_eq!(admin_status, StatusCode::OK);
        let tenant_admin_id = admin_body["id"].as_i64().expect("tenant admin id") as i32;

        let (role_status, role_body) = admin_request(
            "POST",
            &format!("/admin/tenants/{tenant_id}/roles"),
            Some(admin_token()),
            Some(serde_json::json!({
                "name": format!("tenant-admin-role-{suffix}"),
                "description": "Tenant user admin",
                "permissions": ["admin.users.manage"]
            })),
        )
        .await;
        assert_eq!(role_status, StatusCode::OK);
        let role_id = role_body["id"].as_i64().expect("role id") as i32;
        let (assign_status, _) = admin_request(
            "POST",
            &format!("/admin/users/{tenant_admin_id}/roles"),
            Some(admin_token()),
            Some(serde_json::json!({"role_id": role_id})),
        )
        .await;
        assert_eq!(assign_status, StatusCode::OK);

        let managed_email = format!("tenant-managed-{suffix}@example.com");
        let (created_status, created_body) =
            post_tenant_user(Some(admin_token()), tenant_id, &managed_email).await;
        assert_eq!(created_status, StatusCode::OK);
        let managed_user_id = created_body["id"].as_i64().expect("managed user id") as i32;

        let tenant_admin_token = token_for_user(tenant_admin_id);
        let (list_status, users) = admin_request(
            "GET",
            &format!("/admin/tenants/{tenant_id}/users"),
            Some(tenant_admin_token.clone()),
            None,
        )
        .await;
        assert_eq!(list_status, StatusCode::OK);
        assert!(users
            .as_array()
            .unwrap()
            .iter()
            .any(|user| user["id"] == managed_user_id));

        let other_slug = format!("other-tenant-{suffix}");
        let (_, other_tenant) = post_tenant(Some(admin_token()), "Other Tenant", &other_slug).await;
        let other_tenant_id = other_tenant["id"].as_i64().expect("other tenant id") as i32;
        let (forbidden_status, _) = admin_request(
            "GET",
            &format!("/admin/tenants/{other_tenant_id}/users"),
            Some(tenant_admin_token),
            None,
        )
        .await;
        assert_eq!(forbidden_status, StatusCode::FORBIDDEN);
    }
}
