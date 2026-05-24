use std::path::{Path, PathBuf};
use std::sync::Arc;

use bytes::Bytes;
use object_store::aws::AmazonS3Builder;
use object_store::path::Path as ObjectPath;
use object_store::{Attribute, AttributeValue, Attributes, ObjectStore, PutOptions};

use crate::config::AppConfig;

#[derive(Clone)]
pub struct StorageService {
    backend: StorageBackend,
}

#[derive(Clone)]
enum StorageBackend {
    Local {
        root: PathBuf,
    },
    R2 {
        store: Arc<dyn ObjectStore>,
        public_base_url: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredObject {
    pub key: String,
    pub url: String,
    pub size_bytes: usize,
    pub mime_type: String,
}

impl StorageService {
    pub fn from_config(config: &AppConfig) -> Result<Self, String> {
        match config.storage_backend.as_str() {
            "local" => Ok(Self {
                backend: StorageBackend::Local {
                    root: PathBuf::from(&config.storage_local_dir),
                },
            }),
            "r2" | "s3" => {
                let endpoint = required(&config.r2_endpoint, "R2_ENDPOINT")?;
                let access_key_id = required(&config.r2_access_key_id, "R2_ACCESS_KEY_ID")?;
                let secret_access_key =
                    required(&config.r2_secret_access_key, "R2_SECRET_ACCESS_KEY")?;
                let bucket = required(&config.r2_bucket, "R2_BUCKET")?;
                let store = AmazonS3Builder::new()
                    .with_endpoint(endpoint)
                    .with_access_key_id(access_key_id)
                    .with_secret_access_key(secret_access_key)
                    .with_bucket_name(bucket)
                    .build()
                    .map_err(|error| format!("Failed to build R2 storage: {error}"))?;
                Ok(Self {
                    backend: StorageBackend::R2 {
                        store: Arc::new(store),
                        public_base_url: config.r2_public_base_url.clone(),
                    },
                })
            }
            other => Err(format!("Unsupported STORAGE_BACKEND: {other}")),
        }
    }

    pub async fn put(
        &self,
        key: &str,
        bytes: Bytes,
        mime_type: &str,
    ) -> Result<StoredObject, String> {
        validate_key(key)?;
        match &self.backend {
            StorageBackend::Local { root } => {
                let path = safe_local_path(root, key)?;
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|error| format!("Failed to create storage dir: {error}"))?;
                }
                let size_bytes = bytes.len();
                tokio::fs::write(&path, bytes)
                    .await
                    .map_err(|error| format!("Failed to write local object: {error}"))?;
                Ok(StoredObject {
                    key: key.to_string(),
                    url: self.get_url(key).await?,
                    size_bytes,
                    mime_type: mime_type.to_string(),
                })
            }
            StorageBackend::R2 { store, .. } => {
                let size_bytes = bytes.len();
                let path = ObjectPath::from(key);
                let mut attributes = Attributes::new();
                attributes.insert(
                    Attribute::ContentType,
                    AttributeValue::from(mime_type.to_string()),
                );
                store
                    .put_opts(&path, bytes.into(), PutOptions::from(attributes))
                    .await
                    .map_err(|error| format!("Failed to put R2 object: {error}"))?;
                Ok(StoredObject {
                    key: key.to_string(),
                    url: self.get_url(key).await?,
                    size_bytes,
                    mime_type: mime_type.to_string(),
                })
            }
        }
    }

    pub async fn get_url(&self, key: &str) -> Result<String, String> {
        validate_key(key)?;
        match &self.backend {
            StorageBackend::Local { .. } => Ok(format!("/media/{key}")),
            StorageBackend::R2 {
                public_base_url, ..
            } => {
                let Some(base_url) = public_base_url else {
                    return Ok(key.to_string());
                };
                Ok(format!("{}/{}", base_url.trim_end_matches('/'), key))
            }
        }
    }
}

fn required(value: &Option<String>, name: &str) -> Result<String, String> {
    value
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} must be set"))
}

fn validate_key(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("Storage key must not be empty".to_string());
    }
    let path = Path::new(key);
    if path.is_absolute() || key.split('/').any(|part| part == "..") {
        return Err("Storage key must be relative and cannot contain ..".to_string());
    }
    Ok(())
}

fn safe_local_path(root: &Path, key: &str) -> Result<PathBuf, String> {
    validate_key(key)?;
    Ok(root.join(key))
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    fn local_config(dir: String) -> AppConfig {
        AppConfig {
            database_url: "mysql://example".to_string(),
            jwt_secret: "secret".to_string(),
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            app_base_url: "http://localhost:8080".into(),
            storage_backend: "local".to_string(),
            storage_local_dir: dir,
            r2_endpoint: None,
            r2_access_key_id: None,
            r2_secret_access_key: None,
            r2_bucket: None,
            r2_public_base_url: None,
        }
    }

    #[actix_rt::test]
    async fn local_put_writes_file_and_returns_media_url() {
        let dir = std::env::temp_dir().join(format!("crm-bro-storage-{}", uuid::Uuid::new_v4()));
        let service =
            StorageService::from_config(&local_config(dir.to_string_lossy().to_string())).unwrap();
        let stored = service
            .put(
                "tenant-1/inbound/hello.txt",
                Bytes::from_static(b"hello"),
                "text/plain",
            )
            .await
            .unwrap();

        assert_eq!(stored.key, "tenant-1/inbound/hello.txt");
        assert_eq!(stored.url, "/media/tenant-1/inbound/hello.txt");
        assert_eq!(stored.size_bytes, 5);
        assert_eq!(
            tokio::fs::read(dir.join("tenant-1/inbound/hello.txt"))
                .await
                .unwrap(),
            b"hello"
        );
    }

    #[actix_rt::test]
    async fn rejects_path_traversal_keys() {
        let dir = std::env::temp_dir().join(format!("crm-bro-storage-{}", uuid::Uuid::new_v4()));
        let service =
            StorageService::from_config(&local_config(dir.to_string_lossy().to_string())).unwrap();
        let error = service
            .put("../secret.txt", Bytes::from_static(b"no"), "text/plain")
            .await
            .unwrap_err();
        assert!(error.contains("relative"));
    }

    #[test]
    fn r2_config_requires_credentials() {
        let mut config = local_config("media".to_string());
        config.storage_backend = "r2".to_string();
        let error = match StorageService::from_config(&config) {
            Ok(_) => panic!("expected missing R2 config error"),
            Err(error) => error,
        };
        assert!(error.contains("R2_ENDPOINT"));
    }
}
