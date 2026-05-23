pub mod health;
pub mod auth;
pub mod admin;
pub mod chat;

use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .configure(health::configure)
            .configure(auth::configure)
            .configure(admin::configure)
            .configure(chat::configure),
    );
}
