use actix_web::{http::StatusCode, HttpResponse};

use crate::auth::context::AuthContext;

pub fn require_permission(ctx: &AuthContext, permission: &str) -> Result<(), HttpResponse> {
    if ctx.is_superadmin || ctx.permissions.contains(permission) {
        Ok(())
    } else {
        Err(HttpResponse::build(StatusCode::FORBIDDEN).json(serde_json::json!({
            "success": false,
            "error": "Forbidden"
        })))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::auth::context::AuthContext;
    use super::require_permission;

    fn ctx(is_superadmin: bool, permissions: &[&str]) -> AuthContext {
        AuthContext {
            user_id: 1,
            tenant_id: Some(10),
            is_superadmin,
            permissions: permissions.iter().map(|p| p.to_string()).collect::<HashSet<_>>(),
        }
    }

    #[test]
    fn allows_superadmin_without_explicit_permission() {
        assert!(require_permission(&ctx(true, &[]), "contacts.read").is_ok());
    }

    #[test]
    fn allows_user_with_permission() {
        assert!(require_permission(&ctx(false, &["contacts.read"]), "contacts.read").is_ok());
    }

    #[test]
    fn rejects_user_without_permission() {
        let err = require_permission(&ctx(false, &["contacts.write"]), "contacts.read")
            .expect_err("missing permission must reject");
        assert_eq!(err.status(), actix_web::http::StatusCode::FORBIDDEN);
    }
}
