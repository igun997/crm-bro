use crate::domain::auth::errors::AuthError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    id: Option<i32>,
    tenant_id: Option<i32>,
    email: String,
    name: String,
    password_hash: String,
    is_active: bool,
}

impl User {
    pub fn new(
        tenant_id: Option<i32>,
        email: impl Into<String>,
        name: impl Into<String>,
        password_hash: impl Into<String>,
    ) -> Result<Self, AuthError> {
        Self::new_with_optional_tenant(tenant_id, email, name, password_hash)
    }

    pub fn new_superadmin(
        email: impl Into<String>,
        name: impl Into<String>,
        password_hash: impl Into<String>,
    ) -> Result<Self, AuthError> {
        Self::new_with_optional_tenant(None, email, name, password_hash)
    }

    fn new_with_optional_tenant(
        tenant_id: Option<i32>,
        email: impl Into<String>,
        name: impl Into<String>,
        password_hash: impl Into<String>,
    ) -> Result<Self, AuthError> {
        let email = Self::normalize_email(email.into())?;
        let name = Self::validate_name(name.into())?;
        let password_hash = Self::validate_password_hash(password_hash.into())?;

        Ok(Self {
            id: None,
            tenant_id,
            email,
            name,
            password_hash,
            is_active: true,
        })
    }

    fn normalize_email(email: String) -> Result<String, AuthError> {
        let normalized = email.trim().to_lowercase();

        if normalized.len() < 3 || !normalized.contains('@') {
            return Err(AuthError::InvalidEmail(email));
        }

        Ok(normalized)
    }

    fn validate_name(name: String) -> Result<String, AuthError> {
        if name.trim().is_empty() {
            return Err(AuthError::InvalidName(name));
        }

        Ok(name)
    }

    fn validate_password_hash(password_hash: String) -> Result<String, AuthError> {
        if password_hash.trim().is_empty() {
            return Err(AuthError::InvalidPasswordHash);
        }

        Ok(password_hash)
    }

    pub fn id(&self) -> Option<i32> {
        self.id
    }

    pub fn tenant_id(&self) -> Option<i32> {
        self.tenant_id
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn password_hash(&self) -> &str {
        &self.password_hash
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthError, User};

    #[test]
    fn new_superadmin_sets_tenant_id_to_none() {
        let user = User::new_superadmin("root@example.com", "Root", "hash").expect("valid user");

        assert_eq!(user.tenant_id(), None);
    }

    #[test]
    fn new_preserves_tenant_id() {
        let user =
            User::new(Some(42), "ada@example.com", "Ada Lovelace", "hash").expect("valid user");

        assert_eq!(user.tenant_id(), Some(42));
    }

    #[test]
    fn new_rejects_blank_name() {
        let result = User::new(Some(42), "ada@example.com", "   ", "hash");

        assert_eq!(result, Err(AuthError::InvalidName("   ".to_string())));
    }

    #[test]
    fn new_rejects_blank_password_hash() {
        let result = User::new(Some(42), "ada@example.com", "Ada Lovelace", "   ");

        assert_eq!(result, Err(AuthError::InvalidPasswordHash));
    }

    #[test]
    fn new_sets_is_active_to_true() {
        let user =
            User::new(Some(42), "ada@example.com", "Ada Lovelace", "hash").expect("valid user");

        assert!(user.is_active());
    }
}
