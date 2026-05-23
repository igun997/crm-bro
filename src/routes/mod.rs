pub mod admin;
pub mod auth;
pub mod chat;
pub mod contacts;
pub mod health;
pub mod settings;

use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .configure(health::configure)
            .configure(auth::configure)
            .configure(admin::configure)
            .configure(settings::configure)
            .configure(contacts::configure)
            .configure(chat::configure),
    );
}
