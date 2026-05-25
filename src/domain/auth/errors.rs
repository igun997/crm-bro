use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("invalid email: {0}")]
    InvalidEmail(String),
    #[error("invalid name: {0}")]
    InvalidName(String),
    #[error("invalid password hash")]
    InvalidPasswordHash,
    #[error("user not found")]
    UserNotFound,
    #[error("unauthorized")]
    Unauthorized,
    #[error("database error: {0}")]
    Database(String),
}
