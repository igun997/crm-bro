use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};

/// JWT claims used across the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — user id as string
    pub sub: String,
    /// Tenant the user belongs to (None for superadmins without a tenant)
    pub tenant_id: Option<i32>,
    /// Whether the user is a platform superadmin
    pub is_superadmin: bool,
    /// Expiry (Unix timestamp)
    pub exp: usize,
    /// Issued-at (Unix timestamp)
    pub iat: usize,
}

/// Encode a `Claims` struct into a signed JWT string.
pub fn encode_jwt(claims: &Claims, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// Decode and validate a JWT string, returning the inner `Claims`.
pub fn decode_jwt(token: &str, secret: &str) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
}

/// Build a `Claims` value that expires `expires_in_secs` seconds from now.
pub fn build_claims(
    user_id: i32,
    tenant_id: Option<i32>,
    is_superadmin: bool,
    expires_in_secs: i64,
) -> Claims {
    let now = chrono::Utc::now().timestamp() as usize;
    let exp = (chrono::Utc::now() + chrono::Duration::seconds(expires_in_secs)).timestamp() as usize;
    Claims {
        sub: user_id.to_string(),
        tenant_id,
        is_superadmin,
        exp,
        iat: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let claims = build_claims(42, Some(1), false, 3600);
        let secret = "test-secret";
        let token = encode_jwt(&claims, secret).expect("encode");
        let decoded = decode_jwt(&token, secret).expect("decode");
        assert_eq!(decoded.claims.sub, "42");
        assert_eq!(decoded.claims.tenant_id, Some(1));
        assert!(!decoded.claims.is_superadmin);
    }
}
