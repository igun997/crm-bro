use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContactError {
    #[error("Contact not found: {0}")]
    NotFound(i32),
    #[error("Invalid name: {0}")]
    InvalidName(String),
    #[error("Invalid phone: {0}")]
    InvalidPhone(String),
    #[error("Duplicate phone: {0}")]
    DuplicatePhone(String),
    #[error("Database error: {0}")]
    Database(String),
}
