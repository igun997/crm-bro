pub mod hub;
pub mod session;

use actix_web::web;
use actix_web_actors::ws;
use actix_web::{get, HttpRequest, HttpResponse};

use hub::ChatHub;

/// WebSocket endpoint for global updates (all new messages)
#[get("/ws/updates")]
pub async fn ws_updates(
    req: HttpRequest,
    stream: web::Payload,
    hub: web::Data<actix::Addr<ChatHub>>,
) -> Result<HttpResponse, actix_web::Error> {
    let session = session::ChatSession::new(hub.get_ref().clone(), None);
    ws::start(session, &req, stream)
}

/// WebSocket endpoint for specific conversation
#[get("/ws/chat/{conversation_id}")]
pub async fn ws_chat(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<i32>,
    hub: web::Data<actix::Addr<ChatHub>>,
) -> Result<HttpResponse, actix_web::Error> {
    let conversation_id = path.into_inner();
    let session = session::ChatSession::new(hub.get_ref().clone(), Some(conversation_id));
    ws::start(session, &req, stream)
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_updates)
       .service(ws_chat);
}
