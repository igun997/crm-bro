use crate::domain::tenants::TenantError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatsAppSettings {
    id: i32,
    tenant_id: i32,
    phone_number_id: String,
    business_account_id: String,
    display_phone_number: Option<String>,
    access_token: String,
    verify_token: String,
    api_version: String,
    is_active: bool,
}

impl WhatsAppSettings {
    pub fn new(
        tenant_id: i32,
        phone_number_id: String,
        business_account_id: String,
        display_phone_number: Option<String>,
        access_token: String,
        verify_token: String,
        api_version: Option<String>,
        is_active: bool,
    ) -> Result<Self, TenantError> {
        let phone_number_id = phone_number_id.trim().to_string();
        let business_account_id = business_account_id.trim().to_string();
        let access_token = access_token.trim().to_string();
        let verify_token = verify_token.trim().to_string();
        let api_version = api_version.unwrap_or_else(|| "v25.0".to_string());

        if phone_number_id.is_empty() {
            return Err(TenantError::InvalidWhatsAppSettings(
                "phone_number_id is required".into(),
            ));
        }
        if business_account_id.is_empty() {
            return Err(TenantError::InvalidWhatsAppSettings(
                "business_account_id is required".into(),
            ));
        }
        if access_token.is_empty() {
            return Err(TenantError::InvalidWhatsAppSettings(
                "access_token is required".into(),
            ));
        }
        if verify_token.is_empty() {
            return Err(TenantError::InvalidWhatsAppSettings(
                "verify_token is required".into(),
            ));
        }

        Ok(Self {
            id: 0,
            tenant_id,
            phone_number_id,
            business_account_id,
            display_phone_number,
            access_token,
            verify_token,
            api_version,
            is_active,
        })
    }

    pub(crate) fn from_model(model: crate::models::tenant_whatsapp_account::Model) -> Self {
        Self {
            id: model.id,
            tenant_id: model.tenant_id,
            phone_number_id: model.phone_number_id,
            business_account_id: model.business_account_id,
            display_phone_number: model.display_phone_number,
            access_token: model.access_token,
            verify_token: model.verify_token,
            api_version: model.api_version,
            is_active: model.is_active,
        }
    }

    pub fn id(&self) -> i32 {
        self.id
    }
    pub fn tenant_id(&self) -> i32 {
        self.tenant_id
    }
    pub fn phone_number_id(&self) -> &str {
        &self.phone_number_id
    }
    pub fn business_account_id(&self) -> &str {
        &self.business_account_id
    }
    pub fn display_phone_number(&self) -> Option<&str> {
        self.display_phone_number.as_deref()
    }
    pub fn access_token(&self) -> &str {
        &self.access_token
    }
    pub fn verify_token(&self) -> &str {
        &self.verify_token
    }
    pub fn api_version(&self) -> &str {
        &self.api_version
    }
    pub fn is_active(&self) -> bool {
        self.is_active
    }
}
