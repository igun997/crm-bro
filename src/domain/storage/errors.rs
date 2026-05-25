use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Invalid storage config: {0}")]
    InvalidConfig(String),
    #[error("Storage operation failed: {0}")]
    Operation(String),
    #[error("Database error: {0}")]
    Database(String),
}
