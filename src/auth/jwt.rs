use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation, Algorithm};
use serde::{Deserialize, Serialize};

/// JWT claims used across the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — user id as i32
    pub sub: i32,
    /// Tenant the user belongs to (None for superadmins without a tenant)
    pub tenant_id: Option<i32>,
    /// Whether the user is a platform superadmin
    pub is_superadmin: bool,
    /// Expiry (Unix timestamp seconds)
    pub exp: u64,
    /// Issued-at (Unix timestamp seconds)
    pub iat: u64,
}

/// Encode a `Claims` struct into a signed JWT string.
pub fn encode_jwt(claims: &Claims, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    encode(
        &Header::new(Algorithm::HS256),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// Decode and validate a JWT string, returning the inner `Claims`.
/// Validates signature and expiry using explicit HS256.
pub fn decode_jwt(token: &str, secret: &str) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
}

/// Build a `Claims` value that expires `expires_in_secs` seconds from now.
///
/// # Panics
/// Panics if `expires_in_secs` is not positive.
pub fn build_claims(
    user_id: i32,
    tenant_id: Option<i32>,
    is_superadmin: bool,
    expires_in_secs: u64,
) -> Claims {
    assert!(expires_in_secs > 0, "expires_in_secs must be positive");
    let now = chrono::Utc::now().timestamp() as u64;
    Claims {
        sub: user_id,
        tenant_id,
        is_superadmin,
        exp: now + expires_in_secs,
        iat: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-secret-key-for-unit-tests";
    const WRONG_SECRET: &str = "wrong-secret-key";

    #[test]
    fn round_trip_basic() {
        let claims = build_claims(42, Some(1), false, 3600);
        let token = encode_jwt(&claims, SECRET).expect("encode");
        let decoded = decode_jwt(&token, SECRET).expect("decode");
        assert_eq!(decoded.claims.sub, 42);
        assert_eq!(decoded.claims.tenant_id, Some(1));
        assert!(!decoded.claims.is_superadmin);
    }

    #[test]
    fn round_trip_tenant_none_superadmin_true() {
        let claims = build_claims(99, None, true, 3600);
        let token = encode_jwt(&claims, SECRET).expect("encode");
        let decoded = decode_jwt(&token, SECRET).expect("decode");
        assert_eq!(decoded.claims.sub, 99);
        assert_eq!(decoded.claims.tenant_id, None);
        assert!(decoded.claims.is_superadmin);
    }

    #[test]
    fn wrong_secret_rejected() {
        let claims = build_claims(1, None, false, 3600);
        let token = encode_jwt(&claims, SECRET).expect("encode");
        let result = decode_jwt(&token, WRONG_SECRET);
        assert!(result.is_err(), "token signed with different secret must be rejected");
    }

    #[test]
    fn tampered_payload_rejected() {
        let claims = build_claims(1, Some(5), false, 3600);
        let token = encode_jwt(&claims, SECRET).expect("encode");

        // Flip a character in the payload segment (middle part)
        let parts: Vec<&str> = token.splitn(3, '.').collect();
        assert_eq!(parts.len(), 3);
        let mut tampered_payload = parts[1].to_string();
        // Replace last char with a different one to corrupt the payload
        let last = tampered_payload.pop().unwrap_or('A');
        let replacement = if last == 'A' { 'B' } else { 'A' };
        tampered_payload.push(replacement);
        let tampered_token = format!("{}.{}.{}", parts[0], tampered_payload, parts[2]);

        let result = decode_jwt(&tampered_token, SECRET);
        assert!(result.is_err(), "tampered token must be rejected");
    }

    #[test]
    fn expired_token_rejected() {
        // Build a claims that expired 10 seconds ago by manually constructing it
        let now = chrono::Utc::now().timestamp() as u64;
        let claims = Claims {
            sub: 7,
            tenant_id: Some(2),
            is_superadmin: false,
            exp: now - 10, // already expired
            iat: now - 70,
        };
        let token = encode_jwt(&claims, SECRET).expect("encode");
        let result = decode_jwt(&token, SECRET);
        assert!(result.is_err(), "expired token must be rejected");
    }

    #[test]
    #[should_panic(expected = "expires_in_secs must be positive")]
    fn build_claims_zero_expires_panics() {
        build_claims(1, None, false, 0);
    }
}
