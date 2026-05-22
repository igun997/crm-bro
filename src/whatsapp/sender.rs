#![allow(dead_code)]
use reqwest::Client;

use crate::config::AppConfig;
use super::types::*;

pub struct WhatsAppSender {
    client: Client,
    base_url: String,
    access_token: String,
}

impl WhatsAppSender {
    pub fn new(config: &AppConfig) -> Self {
        let base_url = format!(
            "https://graph.facebook.com/{}/{}/messages",
            config.wa_api_version, config.wa_phone_number_id
        );
        Self {
            client: Client::new(),
            base_url,
            access_token: config.wa_access_token.clone(),
        }
    }

    pub async fn send_text(&self, to: &str, body: &str) -> Result<String, String> {
        let payload = SendTextRequest {
            messaging_product: "whatsapp".into(),
            to: to.into(),
            msg_type: "text".into(),
            text: TextBody2 { body: body.into() },
        };

        self.post_message(&payload).await
    }

    pub async fn send_template(
        &self,
        to: &str,
        template_name: &str,
        language: &str,
        components: Option<Vec<TemplateComponent>>,
    ) -> Result<String, String> {
        let payload = SendTemplateRequest {
            messaging_product: "whatsapp".into(),
            to: to.into(),
            msg_type: "template".into(),
            template: TemplateBody {
                name: template_name.into(),
                language: TemplateLanguage { code: language.into() },
                components,
            },
        };

        self.post_message(&payload).await
    }

    pub async fn send_media(
        &self,
        to: &str,
        media_type: &str,
        url: &str,
        caption: Option<&str>,
    ) -> Result<String, String> {
        let media_payload = MediaPayload {
            link: url.into(),
            caption: caption.map(|c| c.into()),
        };

        let payload = SendMediaRequest {
            messaging_product: "whatsapp".into(),
            to: to.into(),
            msg_type: media_type.into(),
            image: if media_type == "image" { Some(media_payload.clone()) } else { None },
            document: if media_type == "document" { Some(media_payload.clone()) } else { None },
            audio: if media_type == "audio" { Some(media_payload.clone()) } else { None },
            video: if media_type == "video" { Some(media_payload) } else { None },
        };

        self.post_message(&payload).await
    }

    async fn post_message<T: serde::Serialize>(&self, payload: &T) -> Result<String, String> {
        let resp = self.client
            .post(&self.base_url)
            .bearer_auth(&self.access_token)
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("Meta API error {}: {}", status, body);
            return Err(format!("Meta API error {}: {}", status, body));
        }

        let result: SendMessageResponse = resp
            .json()
            .await
            .map_err(|e| format!("Parse response failed: {}", e))?;

        result
            .messages
            .and_then(|m| m.first().map(|msg| msg.id.clone()))
            .ok_or_else(|| "No message ID in response".into())
    }
}
