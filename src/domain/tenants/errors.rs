use thiserror::Error;

#[derive(Debug, Error)]
pub enum TenantError {
    #[error("Invalid tenant name: {0}")]
    InvalidName(String),
    #[error("Invalid tenant slug: {0}")]
    InvalidSlug(String),
    #[error("Invalid WhatsApp settings: {0}")]
    InvalidWhatsAppSettings(String),
    #[error("Invalid storage settings: {0}")]
    InvalidStorageSettings(String),
    #[error("Tenant not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(String),
}
