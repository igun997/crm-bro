mod config;
mod response;
mod routes;
mod models;
mod middleware;
mod whatsapp;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer, middleware::Logger};
use sea_orm::Database;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use config::AppConfig;
use routes::health::HealthResponse;
use routes::auth::{LoginRequest, LoginResponse};
use routes::chat::{ConversationResponse, MessageResponse, PaginatedConversations, PaginatedMessages, SendTextBody, SendTemplateBody, SendMediaBody, SendResponse};

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::health::health_check,
        routes::auth::login,
        routes::chat::list_conversations,
        routes::chat::get_messages_by_phone,
        routes::chat::search_messages,
        routes::chat::send_text,
        routes::chat::send_template,
        routes::chat::send_media,
    ),
    components(schemas(
        HealthResponse, LoginRequest, LoginResponse,
        ConversationResponse, MessageResponse,
        PaginatedConversations, PaginatedMessages,
        SendTextBody, SendTemplateBody, SendMediaBody, SendResponse,
    )),
    tags(
        (name = "Health", description = "Health check endpoints"),
        (name = "Auth", description = "Authentication endpoints"),
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

    tracing::info!("Connected to database");
    tracing::info!("Starting server at {}:{}", config.server_host, config.server_port);

    let host = config.server_host.clone();
    let port = config.server_port;

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
            .configure(routes::configure)
            .configure(whatsapp::webhook::configure)
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
    })
    .bind((host, port))?
    .run()
    .await
}
