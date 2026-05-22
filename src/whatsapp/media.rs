#![allow(dead_code)]
use reqwest::Client;
use std::path::PathBuf;
use tokio::fs;

use crate::config::AppConfig;

const MEDIA_DIR: &str = "media";

#[derive(Debug)]
pub struct DownloadedMedia {
    pub local_path: String,
    pub mime_type: String,
}

/// Get media URL from Meta API using media ID
async fn get_media_url(client: &Client, config: &AppConfig, media_id: &str) -> Result<String, String> {
    let url = format!(
        "https://graph.facebook.com/{}/{}",
        config.wa_api_version, media_id
    );

    let resp = client
        .get(&url)
        .bearer_auth(&config.wa_access_token)
        .send()
        .await
        .map_err(|e| format!("Get media URL failed: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Get media URL error: {}", body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse media response: {}", e))?;

    json["url"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No URL in media response".into())
}

/// Download media binary from Meta CDN
async fn download_media_binary(client: &Client, config: &AppConfig, url: &str) -> Result<(Vec<u8>, String), String> {
    let resp = client
        .get(url)
        .bearer_auth(&config.wa_access_token)
        .send()
        .await
        .map_err(|e| format!("Download media failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download media error: {}", resp.status()));
    }

    let mime = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Read media bytes: {}", e))?;

    Ok((bytes.to_vec(), mime))
}

/// Download media by ID and save to local disk
pub async fn download_and_save(
    config: &AppConfig,
    media_id: &str,
    conversation_id: i32,
) -> Result<DownloadedMedia, String> {
    let client = Client::new();

    // 1. Get download URL from Meta
    let media_url = get_media_url(&client, config, media_id).await?;

    // 2. Download binary
    let (bytes, mime_type) = download_media_binary(&client, config, &media_url).await?;

    // 3. Determine extension from mime
    let ext = mime_to_extension(&mime_type);

    // 4. Save to disk
    let dir = PathBuf::from(MEDIA_DIR).join(conversation_id.to_string());
    fs::create_dir_all(&dir).await.map_err(|e| format!("Create dir: {}", e))?;

    let filename = format!("{}_{}.{}", media_id, chrono::Utc::now().timestamp(), ext);
    let filepath = dir.join(&filename);

    fs::write(&filepath, &bytes).await.map_err(|e| format!("Write file: {}", e))?;

    let local_path = filepath.to_string_lossy().to_string();
    tracing::info!("Downloaded media {} → {}", media_id, local_path);

    Ok(DownloadedMedia { local_path, mime_type })
}

fn mime_to_extension(mime: &str) -> &str {
    match mime {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "audio/ogg" | "audio/ogg; codecs=opus" => "ogg",
        "audio/mpeg" => "mp3",
        "audio/aac" => "aac",
        "video/mp4" => "mp4",
        "video/3gpp" => "3gp",
        "application/pdf" => "pdf",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        _ => "bin",
    }
}
