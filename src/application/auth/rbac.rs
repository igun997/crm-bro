use actix_web::{http::StatusCode, HttpResponse};

use crate::api::middleware::AuthContext;

pub fn require_permission(ctx: &AuthContext, permission: &str) -> Result<(), HttpResponse> {
    if ctx.is_superadmin || ctx.permissions.contains(permission) {
        Ok(())
    } else {
        Err(
            HttpResponse::build(StatusCode::FORBIDDEN).json(serde_json::json!({
                "success": false,
                "error": "Forbidden"
            })),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::require_permission;
    use crate::api::middleware::AuthContext;
    use crate::domain::auth::permissions::{self, default_tenant_roles, roles, PERMISSIONS};

    fn ctx(is_superadmin: bool, permissions: &[&str]) -> AuthContext {
        AuthContext {
            user_id: 1,
            tenant_id: Some(10),
            is_superadmin,
            permissions: permissions
                .iter()
                .map(|p| p.to_string())
                .collect::<HashSet<_>>(),
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

    #[test]
    fn exposes_permission_constants_for_middleware_checks() {
        assert_eq!(permissions::ADMIN_USERS_MANAGE, "admin.users.manage");
        assert!(PERMISSIONS
            .iter()
            .any(|permission| permission.code == permissions::ADMIN_USERS_MANAGE));
        assert!(require_permission(
            &ctx(false, &[permissions::ADMIN_USERS_MANAGE]),
            permissions::ADMIN_USERS_MANAGE
        )
        .is_ok());
    }

    #[test]
    fn exposes_seeded_default_tenant_roles() {
        let defaults = default_tenant_roles();
        let tenant_admin = defaults
            .iter()
            .find(|role| role.name == roles::TENANT_ADMIN)
            .expect("tenant_admin role exists");
        assert!(tenant_admin
            .permissions
            .contains(&permissions::ADMIN_USERS_MANAGE));

        let agent = defaults
            .iter()
            .find(|role| role.name == roles::AGENT)
            .expect("agent role exists");
        assert!(agent.permissions.contains(&permissions::CHATS_SEND));
        assert!(!agent.permissions.contains(&permissions::ADMIN_USERS_MANAGE));

        let viewer = defaults
            .iter()
            .find(|role| role.name == roles::VIEWER)
            .expect("viewer role exists");
        assert_eq!(
            viewer.permissions,
            &[permissions::CONTACTS_READ, permissions::CHATS_READ]
        );
    }
}
