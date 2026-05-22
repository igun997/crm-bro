#![allow(dead_code)]
use serde::{Deserialize, Serialize};

// === Inbound (from Meta webhook) ===

#[derive(Debug, Deserialize)]
pub struct WebhookPayload {
    pub entry: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
pub struct Entry {
    pub id: String,
    pub changes: Vec<Change>,
}

#[derive(Debug, Deserialize)]
pub struct Change {
    pub value: ChangeValue,
    pub field: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangeValue {
    pub messaging_product: Option<String>,
    pub metadata: Option<Metadata>,
    pub contacts: Option<Vec<Contact>>,
    pub messages: Option<Vec<InboundMessage>>,
    pub statuses: Option<Vec<StatusUpdate>>,
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub display_phone_number: String,
    pub phone_number_id: String,
}

#[derive(Debug, Deserialize)]
pub struct Contact {
    pub profile: ContactProfile,
    pub wa_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ContactProfile {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct InboundMessage {
    pub from: String,
    pub id: String,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub text: Option<TextBody>,
    pub image: Option<MediaInfo>,
    pub document: Option<MediaInfo>,
    pub audio: Option<MediaInfo>,
    pub video: Option<MediaInfo>,
}

#[derive(Debug, Deserialize)]
pub struct TextBody {
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct MediaInfo {
    pub id: String,
    pub mime_type: Option<String>,
    pub caption: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StatusUpdate {
    pub id: String,
    pub status: String,
    pub timestamp: String,
    pub recipient_id: String,
}

// === Outbound (to Meta API) ===

#[derive(Debug, Serialize)]
pub struct SendTextRequest {
    pub messaging_product: String,
    pub to: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub text: TextBody2,
}

#[derive(Debug, Serialize)]
pub struct TextBody2 {
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct SendTemplateRequest {
    pub messaging_product: String,
    pub to: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub template: TemplateBody,
}

#[derive(Debug, Serialize)]
pub struct TemplateBody {
    pub name: String,
    pub language: TemplateLanguage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<TemplateComponent>>,
}

#[derive(Debug, Serialize)]
pub struct TemplateLanguage {
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct TemplateComponent {
    #[serde(rename = "type")]
    pub comp_type: String,
    pub parameters: Vec<TemplateParameter>,
}

#[derive(Debug, Serialize)]
pub struct TemplateParameter {
    #[serde(rename = "type")]
    pub param_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendMediaRequest {
    pub messaging_product: String,
    pub to: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<MediaPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<MediaPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<MediaPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<MediaPayload>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MediaPayload {
    pub link: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

// === Meta API Response ===

#[derive(Debug, Deserialize)]
pub struct SendMessageResponse {
    pub messaging_product: Option<String>,
    pub contacts: Option<Vec<ResponseContact>>,
    pub messages: Option<Vec<ResponseMessage>>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseContact {
    pub input: String,
    pub wa_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    pub id: String,
}
