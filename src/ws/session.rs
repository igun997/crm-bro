use actix::prelude::*;
use actix_web_actors::ws;
use std::time::{Duration, Instant};

use super::hub::{ChatHub, ChatMessage, Connect, Disconnect};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

static mut NEXT_ID: usize = 0;

fn next_id() -> usize {
    unsafe {
        NEXT_ID += 1;
        NEXT_ID
    }
}

pub struct ChatSession {
    pub id: usize,
    pub hub: Addr<ChatHub>,
    pub conversation_id: Option<i32>,
    pub hb: Instant,
}

impl ChatSession {
    pub fn new(hub: Addr<ChatHub>, conversation_id: Option<i32>) -> Self {
        Self {
            id: next_id(),
            hub,
            conversation_id,
            hb: Instant::now(),
        }
    }

    fn heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                tracing::warn!("WS heartbeat timeout, disconnecting {}", act.id);
                act.hub.do_send(Disconnect { id: act.id });
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }
}

impl Actor for ChatSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.heartbeat(ctx);

        let addr = ctx.address();
        self.hub.do_send(Connect {
            addr: addr.recipient(),
            id: self.id,
            conversation_id: self.conversation_id,
        });
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        self.hub.do_send(Disconnect { id: self.id });
        Running::Stop
    }
}

/// Handle incoming WebSocket messages
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for ChatSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(_text)) => {
                // Could handle client commands here in future
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}

/// Receive broadcast messages from hub
impl Handler<ChatMessage> for ChatSession {
    type Result = ();

    fn handle(&mut self, msg: ChatMessage, ctx: &mut Self::Context) {
        let json = serde_json::to_string(&msg).unwrap_or_default();
        ctx.text(json);
    }
}
