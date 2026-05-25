use actix_web::web;
use crm_bro::api::routes::{admin, auth, chat, contacts, health, settings};

#[test]
fn api_routes_exports_route_modules_and_configure() {
    let _ = admin::configure as fn(&mut web::ServiceConfig);
    let _ = auth::configure as fn(&mut web::ServiceConfig);
    let _ = chat::configure as fn(&mut web::ServiceConfig);
    let _ = contacts::configure as fn(&mut web::ServiceConfig);
    let _ = health::configure as fn(&mut web::ServiceConfig);
    let _ = settings::configure as fn(&mut web::ServiceConfig);
    let _ = crm_bro::api::routes::configure as fn(&mut web::ServiceConfig);
}
