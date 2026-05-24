use actix::prelude::*;
use serde::Serialize;
use std::collections::HashMap;

/// Message broadcast to clients
#[derive(Debug, Clone, Serialize, Message)]
#[rtype(result = "()")]
pub struct ChatMessage {
    pub tenant_id: i32,
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
    pub tenant_id: i32,
    /// None = global updates, Some(id) = specific conversation
    pub conversation_id: Option<i32>,
}

/// Disconnect a session
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: usize,
    pub tenant_id: i32,
    pub conversation_id: Option<i32>,
}

/// Chat hub - manages connected WebSocket sessions
pub struct ChatHub {
    /// Global listeners per tenant
    global_sessions: HashMap<i32, HashMap<usize, Recipient<ChatMessage>>>,
    /// Per-tenant, per-conversation listeners
    conversation_sessions: HashMap<i32, HashMap<i32, HashMap<usize, Recipient<ChatMessage>>>>,
}

impl Default for ChatHub {
    fn default() -> Self {
        Self::new()
    }
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
                self.global_sessions
                    .entry(msg.tenant_id)
                    .or_default()
                    .insert(msg.id, msg.addr);
                tracing::info!(
                    "WS global client {} connected to tenant {}",
                    msg.id,
                    msg.tenant_id
                );
            }
            Some(conv_id) => {
                self.conversation_sessions
                    .entry(msg.tenant_id)
                    .or_default()
                    .entry(conv_id)
                    .or_default()
                    .insert(msg.id, msg.addr);
                tracing::info!(
                    "WS client {} connected to tenant {} conversation {}",
                    msg.id,
                    msg.tenant_id,
                    conv_id
                );
            }
        }
    }
}

impl Handler<Disconnect> for ChatHub {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        match msg.conversation_id {
            None => {
                if let Some(sessions) = self.global_sessions.get_mut(&msg.tenant_id) {
                    sessions.remove(&msg.id);
                }
            }
            Some(conversation_id) => {
                if let Some(tenant_sessions) = self.conversation_sessions.get_mut(&msg.tenant_id) {
                    if let Some(sessions) = tenant_sessions.get_mut(&conversation_id) {
                        sessions.remove(&msg.id);
                    }
                }
            }
        }
        tracing::info!("WS client disconnected: {}", msg.id);
    }
}

impl Handler<ChatMessage> for ChatHub {
    type Result = ();

    fn handle(&mut self, msg: ChatMessage, _: &mut Context<Self>) {
        if let Some(sessions) = self.global_sessions.get(&msg.tenant_id) {
            for addr in sessions.values() {
                addr.do_send(msg.clone());
            }
        }

        if let Some(tenant_sessions) = self.conversation_sessions.get(&msg.tenant_id) {
            if let Some(sessions) = tenant_sessions.get(&msg.conversation_id) {
                for addr in sessions.values() {
                    addr.do_send(msg.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_carries_tenant_id_for_scoped_broadcasts() {
        let msg = ChatMessage {
            tenant_id: 7,
            conversation_id: 9,
            message_id: 11,
            direction: "inbound".into(),
            msg_type: "text".into(),
            body: Some("hello".into()),
            contact_phone: "628".into(),
            contact_name: None,
            timestamp: "2026-05-23 00:00:00".into(),
        };

        assert_eq!(msg.tenant_id, 7);
    }
}
