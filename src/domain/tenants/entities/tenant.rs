use crate::domain::tenants::TenantError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tenant {
    id: i32,
    name: String,
    slug: String,
    is_active: bool,
}

impl Tenant {
    pub fn new(name: String, slug: String) -> Result<Self, TenantError> {
        let name = name.trim().to_string();
        let slug = slug.trim().to_lowercase();

        if name.is_empty() {
            return Err(TenantError::InvalidName("name cannot be empty".into()));
        }
        if slug.is_empty() {
            return Err(TenantError::InvalidSlug("slug cannot be empty".into()));
        }
        if !slug
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        {
            return Err(TenantError::InvalidSlug(
                "slug must contain lowercase letters, digits, or hyphens".into(),
            ));
        }
        if slug.starts_with('-') || slug.ends_with('-') || slug.contains("--") {
            return Err(TenantError::InvalidSlug(
                "slug cannot start/end with hyphen or contain double hyphen".into(),
            ));
        }

        Ok(Self {
            id: 0,
            name,
            slug,
            is_active: true,
        })
    }

    pub(crate) fn from_model(model: crate::models::tenant::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            slug: model.slug,
            is_active: model.is_active,
        }
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn slug(&self) -> &str {
        &self.slug
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tenant_rejects_empty_name() {
        let result = Tenant::new(" ".into(), "acme".into());

        assert!(matches!(result, Err(TenantError::InvalidName(_))));
    }

    #[test]
    fn new_tenant_rejects_invalid_slug() {
        let result = Tenant::new("Acme".into(), "Acme Corp".into());

        assert!(matches!(result, Err(TenantError::InvalidSlug(_))));
    }

    #[test]
    fn new_tenant_normalizes_name_and_slug() {
        let tenant = Tenant::new(" Acme Corp ".into(), "ACME".into()).unwrap();

        assert_eq!(tenant.name(), "Acme Corp");
        assert_eq!(tenant.slug(), "acme");
        assert!(tenant.is_active());
    }
}
