pub mod context;
pub mod extractor;
pub mod jwt;
pub mod password;

/// Pull the JWT secret from `AppConfig`, with a fallback for tests.
/// In production `AppConfig::jwt_secret` is always set (panics at startup if missing).
pub use context::AuthContext;
pub use extractor::CurrentUser;
pub use jwt::{build_claims, decode_jwt, encode_jwt, Claims};
pub use password::{hash_password, verify_password};
