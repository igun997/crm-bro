#![allow(dead_code)]
use reqwest::multipart;
use reqwest::Client;

use super::types::*;

pub struct WhatsAppSender {
    client: Client,
    base_url: String,
    media_url: String,
    access_token: String,
}

impl WhatsAppSender {
    pub fn from_parts(api_version: &str, phone_number_id: &str, access_token: &str) -> Self {
        let base_url = format!(
            "https://graph.facebook.com/{}/{}/messages",
            api_version, phone_number_id
        );
        let media_url = format!(
            "https://graph.facebook.com/{}/{}/media",
            api_version, phone_number_id
        );
        Self {
            client: Client::new(),
            base_url,
            media_url,
            access_token: access_token.to_string(),
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
                language: TemplateLanguage {
                    code: language.into(),
                },
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
            image: if media_type == "image" {
                Some(media_payload.clone())
            } else {
                None
            },
            document: if media_type == "document" {
                Some(media_payload.clone())
            } else {
                None
            },
            audio: if media_type == "audio" {
                Some(media_payload.clone())
            } else {
                None
            },
            video: if media_type == "video" {
                Some(media_payload)
            } else {
                None
            },
        };

        self.post_message(&payload).await
    }

    async fn post_message<T: serde::Serialize>(&self, payload: &T) -> Result<String, String> {
        let resp = self
            .client
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

    /// Upload media to Meta and get media_id
    pub async fn upload_media(&self, file_path: &str, mime_type: &str) -> Result<String, String> {
        let file_bytes = tokio::fs::read(file_path)
            .await
            .map_err(|e| format!("Read file failed: {}", e))?;

        let filename = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();

        let file_part = multipart::Part::bytes(file_bytes)
            .file_name(filename)
            .mime_str(mime_type)
            .map_err(|e| format!("Mime error: {}", e))?;

        let form = multipart::Form::new()
            .text("messaging_product", "whatsapp")
            .text("type", mime_type.to_string())
            .part("file", file_part);

        let resp = self
            .client
            .post(&self.media_url)
            .bearer_auth(&self.access_token)
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("Upload failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Upload error: {}", body));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse upload response: {}", e))?;

        json["id"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No media ID in upload response".into())
    }

    /// Send media using uploaded media_id
    pub async fn send_media_by_id(
        &self,
        to: &str,
        media_type: &str,
        media_id: &str,
        caption: Option<&str>,
    ) -> Result<String, String> {
        let media_obj = serde_json::json!({
            "id": media_id,
            "caption": caption
        });

        let payload = serde_json::json!({
            "messaging_product": "whatsapp",
            "to": to,
            "type": media_type,
            media_type: media_obj
        });

        self.post_message(&payload).await
    }
}
