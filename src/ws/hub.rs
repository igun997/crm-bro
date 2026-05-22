use actix::prelude::*;
use serde::Serialize;
use std::collections::HashMap;

/// Message broadcast to clients
#[derive(Debug, Clone, Serialize, Message)]
#[rtype(result = "()")]
pub struct ChatMessage {
    pub conversation_id: i32,
    pub message_id: i32,
    pub direction: String,
    pub msg_type: String,
    pub body: Option<String>,
    pub contact_phone: String,
    pub contact_name: Option<String>,
    pub timestamp: String,
}

/// Connect a new session
#[derive(Message)]
#[rtype(result = "()")]
pub struct Connect {
    pub addr: Recipient<ChatMessage>,
    pub id: usize,
    /// None = global updates, Some(id) = specific conversation
    pub conversation_id: Option<i32>,
}

/// Disconnect a session
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: usize,
}

/// Chat hub - manages connected WebSocket sessions
pub struct ChatHub {
    /// Global listeners (all messages)
    global_sessions: HashMap<usize, Recipient<ChatMessage>>,
    /// Per-conversation listeners
    conversation_sessions: HashMap<i32, HashMap<usize, Recipient<ChatMessage>>>,
}

impl ChatHub {
    pub fn new() -> Self {
        Self {
            global_sessions: HashMap::new(),
            conversation_sessions: HashMap::new(),
        }
    }
}

impl Actor for ChatHub {
    type Context = Context<Self>;
}

impl Handler<Connect> for ChatHub {
    type Result = ();

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) {
        match msg.conversation_id {
            None => {
                self.global_sessions.insert(msg.id, msg.addr);
                tracing::info!("WS global client connected: {}", msg.id);
            }
            Some(conv_id) => {
                self.conversation_sessions
                    .entry(conv_id)
                    .or_insert_with(HashMap::new)
                    .insert(msg.id, msg.addr);
                tracing::info!("WS client {} connected to conversation {}", msg.id, conv_id);
            }
        }
    }
}

impl Handler<Disconnect> for ChatHub {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        self.global_sessions.remove(&msg.id);
        // Remove from all conversation sessions
        for sessions in self.conversation_sessions.values_mut() {
            sessions.remove(&msg.id);
        }
        tracing::info!("WS client disconnected: {}", msg.id);
    }
}

impl Handler<ChatMessage> for ChatHub {
    type Result = ();

    fn handle(&mut self, msg: ChatMessage, _: &mut Context<Self>) {
        // Broadcast to global listeners
        for addr in self.global_sessions.values() {
            let _ = addr.do_send(msg.clone());
        }

        // Broadcast to conversation-specific listeners
        if let Some(sessions) = self.conversation_sessions.get(&msg.conversation_id) {
            for addr in sessions.values() {
                let _ = addr.do_send(msg.clone());
            }
        }
    }
}
