#![allow(dead_code)]
use reqwest::Client;

#[derive(Debug)]
pub struct DownloadedMediaBytes {
    pub bytes: bytes::Bytes,
    pub mime_type: String,
}

pub async fn get_media_url_with_token(client: &Client, api_version: &str, access_token: &str, media_id: &str) -> Result<String, String> {
    let url = format!("https://graph.facebook.com/{}/{}", api_version, media_id);

    let resp = client
        .get(&url)
        .bearer_auth(access_token)
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

pub async fn download_media_binary_with_token(client: &Client, access_token: &str, url: &str) -> Result<(bytes::Bytes, String), String> {
    let resp = client
        .get(url)
        .bearer_auth(access_token)
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

    Ok((bytes, mime))
}

pub async fn download_bytes(
    api_version: &str,
    access_token: &str,
    media_id: &str,
) -> Result<DownloadedMediaBytes, String> {
    let client = Client::new();
    let media_url = get_media_url_with_token(&client, api_version, access_token, media_id).await?;
    let (bytes, mime_type) = download_media_binary_with_token(&client, access_token, &media_url).await?;
    Ok(DownloadedMediaBytes { bytes, mime_type })
}

pub fn mime_to_extension(mime: &str) -> &str {
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
