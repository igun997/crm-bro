use super::errors::StorageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageBackendKind {
    Local,
    R2,
    S3,
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub backend: StorageBackendKind,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub region: Option<String>,
    pub public_base_url: Option<String>,
    pub local_dir: Option<String>,
}

pub struct StorageConfigFactory;

impl StorageConfigFactory {
    pub fn local(local_dir: impl Into<String>) -> Result<StorageConfig, StorageError> {
        let local_dir = local_dir.into();
        if local_dir.trim().is_empty() {
            return Err(StorageError::InvalidConfig("local_dir is required".into()));
        }

        Ok(StorageConfig {
            backend: StorageBackendKind::Local,
            endpoint: None,
            bucket: None,
            access_key_id: None,
            secret_access_key: None,
            region: None,
            public_base_url: None,
            local_dir: Some(local_dir),
        })
    }

    pub fn r2(
        endpoint: impl Into<String>,
        bucket: impl Into<String>,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
        public_base_url: Option<String>,
    ) -> Result<StorageConfig, StorageError> {
        Self::object_storage(
            StorageBackendKind::R2,
            endpoint,
            bucket,
            access_key_id,
            secret_access_key,
            Some("auto".to_string()),
            public_base_url,
        )
    }

    pub fn object_storage(
        backend: StorageBackendKind,
        endpoint: impl Into<String>,
        bucket: impl Into<String>,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
        region: Option<String>,
        public_base_url: Option<String>,
    ) -> Result<StorageConfig, StorageError> {
        let endpoint = endpoint.into();
        let bucket = bucket.into();
        let access_key_id = access_key_id.into();
        let secret_access_key = secret_access_key.into();

        if endpoint.trim().is_empty() {
            return Err(StorageError::InvalidConfig("endpoint is required".into()));
        }
        if bucket.trim().is_empty() {
            return Err(StorageError::InvalidConfig("bucket is required".into()));
        }
        if access_key_id.trim().is_empty() {
            return Err(StorageError::InvalidConfig(
                "access_key_id is required".into(),
            ));
        }
        if secret_access_key.trim().is_empty() {
            return Err(StorageError::InvalidConfig(
                "secret_access_key is required".into(),
            ));
        }

        Ok(StorageConfig {
            backend,
            endpoint: Some(endpoint),
            bucket: Some(bucket),
            access_key_id: Some(access_key_id),
            secret_access_key: Some(secret_access_key),
            region,
            public_base_url,
            local_dir: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_storage_config_requires_directory() {
        let result = StorageConfigFactory::local("");
        assert!(matches!(result, Err(StorageError::InvalidConfig(_))));
    }

    #[test]
    fn r2_storage_config_requires_bucket() {
        let result = StorageConfigFactory::r2("endpoint", "", "key", "secret", None);
        assert!(matches!(result, Err(StorageError::InvalidConfig(_))));
    }
}
