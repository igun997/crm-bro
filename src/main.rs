use crm_bro::api::dto::contacts::{
    AttachTagRequest, ContactResponse, CreateTagRequest, PaginatedContacts, PatchContactRequest,
    TagResponse,
};
use crm_bro::api::routes;
use crm_bro::infrastructure::config::AppConfig;
use crm_bro::infrastructure::storage::StorageService;
use crm_bro::whatsapp;
use crm_bro::ws;

use actix::Actor;
use actix_cors::Cors;
use actix_files as afiles;
use actix_web::{middleware::Logger, web, App, HttpServer};
use sea_orm::Database;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use routes::admin::{
    AdminUserResponse, AssignRoleRequest, CreateRoleRequest, CreateTenantRequest,
    CreateTenantUserRequest, PatchRoleRequest, PatchUserRequest, PermissionResponse,
    ResetPasswordRequest, RoleResponse, TenantResponse,
};
use routes::auth::{LoginRequest, LoginResponse, LoginUser};
use routes::chat::{
    ConversationResponse, MessageResponse, PaginatedConversations, PaginatedMessages,
    SendMediaBody, SendResponse, SendTemplateBody, SendTextBody,
};
use routes::health::HealthResponse;
use routes::settings::{
    CreateStorageConfigRequest, PatchStorageConfigRequest, PatchWhatsAppAccountRequest,
    StorageConfigResponse, UpsertWhatsAppAccountRequest, WhatsAppAccountResponse,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::health::health_check,
        routes::auth::login,
        routes::admin::create_tenant,
        routes::admin::create_tenant_user,
        routes::admin::list_tenant_users,
        routes::admin::get_user,
        routes::admin::update_user,
        routes::admin::reset_user_password,
        routes::admin::assign_user_role,
        routes::admin::remove_user_role,
        routes::admin::list_tenant_roles,
        routes::admin::create_tenant_role,
        routes::admin::update_role,
        routes::admin::list_permissions,
        routes::settings::list_whatsapp_accounts,
        routes::settings::create_whatsapp_account,
        routes::settings::update_whatsapp_account,
        routes::settings::get_storage_config,
        routes::settings::create_storage_config,
        routes::settings::update_storage_config,
        routes::contacts::list_contacts,
        routes::contacts::get_contact,
        routes::contacts::update_contact,
        routes::contacts::attach_tag,
        routes::contacts::detach_tag,
        routes::contacts::list_tags,
        routes::contacts::create_tag,
        routes::chat::list_conversations,
        routes::chat::get_messages_by_phone,
        routes::chat::search_messages,
        routes::chat::send_text,
        routes::chat::send_template,
        routes::chat::send_media,
        routes::chat::send_upload,
    ),
    components(schemas(
        HealthResponse, LoginRequest, LoginResponse, LoginUser,
        CreateTenantRequest, TenantResponse, CreateTenantUserRequest, AdminUserResponse,
        PatchUserRequest, ResetPasswordRequest, AssignRoleRequest,
        RoleResponse, CreateRoleRequest, PatchRoleRequest, PermissionResponse,
        WhatsAppAccountResponse, UpsertWhatsAppAccountRequest, PatchWhatsAppAccountRequest,
        StorageConfigResponse, CreateStorageConfigRequest, PatchStorageConfigRequest,
        ContactResponse, TagResponse, PaginatedContacts, PatchContactRequest, CreateTagRequest,
        AttachTagRequest,
        ConversationResponse, MessageResponse,
        PaginatedConversations, PaginatedMessages,
        SendTextBody, SendTemplateBody, SendMediaBody, SendResponse,
    )),
    tags(
        (name = "Health", description = "Health check endpoints"),
        (name = "Auth", description = "Authentication endpoints"),
        (name = "Admin", description = "Tenant and user administration endpoints"),
        (name = "Settings", description = "Tenant settings endpoints"),
        (name = "Contacts", description = "Tenant contacts and tags endpoints"),
        (name = "Chat", description = "WhatsApp chat endpoints"),
    ),
    info(title = "CRM Bro API", version = "0.1.0")
)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = AppConfig::from_env();
    let db = Database::connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    let storage =
        StorageService::from_config(&config).expect("Failed to initialize storage service");

    tracing::info!("Connected to database");
    tracing::info!(
        "Starting server at {}:{}",
        config.server_host,
        config.server_port
    );

    let host = config.server_host.clone();
    let port = config.server_port;
    let media_dir = config.storage_local_dir.clone();

    // Start WebSocket hub
    let hub = ws::hub::ChatHub::new().start();

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(Logger::default())
            .app_data(web::Data::new(db.clone()))
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(storage.clone()))
            .app_data(web::Data::new(hub.clone()))
            .configure(routes::configure)
            .configure(whatsapp::webhook::configure)
            .configure(ws::configure)
            .service(afiles::Files::new("/static", "./static").index_file("index.html"))
            .service(afiles::Files::new("/media", media_dir.clone()))
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
    })
    .bind((host, port))?
    .run()
    .await
}
