use crate::domain::{storage::StorageConfigFactory, tenants::TenantError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageSettings {
    id: i32,
    tenant_id: i32,
    endpoint: String,
    region: String,
    access_key_id: String,
    secret_access_key: String,
    bucket: String,
    public_base_url: Option<String>,
    is_active: bool,
}

impl StorageSettings {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant_id: i32,
        endpoint: String,
        region: Option<String>,
        access_key_id: String,
        secret_access_key: String,
        bucket: String,
        public_base_url: Option<String>,
        is_active: bool,
    ) -> Result<Self, TenantError> {
        let config = StorageConfigFactory::r2(
            endpoint,
            bucket,
            access_key_id,
            secret_access_key,
            public_base_url.clone(),
        )
        .map_err(|error| TenantError::InvalidStorageSettings(error.to_string()))?;

        Ok(Self {
            id: 0,
            tenant_id,
            endpoint: config.endpoint.unwrap_or_default(),
            region: region
                .or(config.region)
                .unwrap_or_else(|| "auto".to_string()),
            access_key_id: config.access_key_id.unwrap_or_default(),
            secret_access_key: config.secret_access_key.unwrap_or_default(),
            bucket: config.bucket.unwrap_or_default(),
            public_base_url,
            is_active,
        })
    }

    pub(crate) fn from_model(model: crate::models::tenant_storage_config::Model) -> Self {
        Self {
            id: model.id,
            tenant_id: model.tenant_id,
            endpoint: model.endpoint,
            region: model.region,
            access_key_id: model.access_key_id,
            secret_access_key: model.secret_access_key,
            bucket: model.bucket,
            public_base_url: model.public_base_url,
            is_active: model.is_active,
        }
    }

    pub fn id(&self) -> i32 {
        self.id
    }
    pub fn tenant_id(&self) -> i32 {
        self.tenant_id
    }
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    pub fn region(&self) -> &str {
        &self.region
    }
    pub fn access_key_id(&self) -> &str {
        &self.access_key_id
    }
    pub fn secret_access_key(&self) -> &str {
        &self.secret_access_key
    }
    pub fn bucket(&self) -> &str {
        &self.bucket
    }
    pub fn public_base_url(&self) -> Option<&str> {
        self.public_base_url.as_deref()
    }
    pub fn is_active(&self) -> bool {
        self.is_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_storage_settings_rejects_missing_endpoint() {
        let result = StorageSettings::new(
            1,
            " ".into(),
            None,
            "access-key".into(),
            "secret-key".into(),
            "bucket".into(),
            None,
            true,
        );

        assert!(matches!(
            result,
            Err(TenantError::InvalidStorageSettings(_))
        ));
    }

    #[test]
    fn new_storage_settings_defaults_region_to_auto() {
        let settings = StorageSettings::new(
            1,
            "https://example.r2.cloudflarestorage.com".into(),
            None,
            "access-key".into(),
            "secret-key".into(),
            "bucket".into(),
            Some("https://cdn.example.com".into()),
            true,
        )
        .unwrap();

        assert_eq!(settings.region(), "auto");
        assert_eq!(settings.id(), 0);
    }
}
