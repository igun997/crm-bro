use actix_web::{http::StatusCode, HttpResponse};

use crate::auth::context::AuthContext;

pub mod permissions {
    pub const CHATS_READ: &str = "chats.read";
    pub const CHATS_SEND: &str = "chats.send";
    pub const CONTACTS_READ: &str = "contacts.read";
    pub const CONTACTS_WRITE: &str = "contacts.write";
    pub const CONTACTS_DELETE: &str = "contacts.delete";
    pub const TAGS_READ: &str = "tags.read";
    pub const TAGS_WRITE: &str = "tags.write";
    pub const SETTINGS_WHATSAPP_MANAGE: &str = "settings.whatsapp.manage";
    pub const ADMIN_TENANTS_MANAGE: &str = "admin.tenants.manage";
    pub const ADMIN_USERS_MANAGE: &str = "admin.users.manage";
}

pub mod roles {
    pub const SUPERADMIN: &str = "superadmin";
    pub const TENANT_ADMIN: &str = "tenant_admin";
    pub const AGENT: &str = "agent";
    pub const VIEWER: &str = "viewer";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermissionDef {
    pub code: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoleDef {
    pub name: &'static str,
    pub description: &'static str,
    pub permissions: &'static [&'static str],
}

pub const PERMISSIONS: &[PermissionDef] = &[
    PermissionDef {
        code: permissions::CHATS_READ,
        description: "Read conversations and messages",
    },
    PermissionDef {
        code: permissions::CHATS_SEND,
        description: "Send messages",
    },
    PermissionDef {
        code: permissions::CONTACTS_READ,
        description: "Read contacts",
    },
    PermissionDef {
        code: permissions::CONTACTS_WRITE,
        description: "Create and update contacts",
    },
    PermissionDef {
        code: permissions::CONTACTS_DELETE,
        description: "Delete contacts",
    },
    PermissionDef {
        code: permissions::TAGS_READ,
        description: "Read tags",
    },
    PermissionDef {
        code: permissions::TAGS_WRITE,
        description: "Create and update tags",
    },
    PermissionDef {
        code: permissions::SETTINGS_WHATSAPP_MANAGE,
        description: "Manage WhatsApp account settings",
    },
    PermissionDef {
        code: permissions::ADMIN_TENANTS_MANAGE,
        description: "Create and manage tenants",
    },
    PermissionDef {
        code: permissions::ADMIN_USERS_MANAGE,
        description: "Create and manage users",
    },
];

pub const SUPERADMIN_ROLE: RoleDef = RoleDef {
    name: roles::SUPERADMIN,
    description: "System superadmin role with all permissions",
    permissions: &[
        permissions::CHATS_READ,
        permissions::CHATS_SEND,
        permissions::CONTACTS_READ,
        permissions::CONTACTS_WRITE,
        permissions::CONTACTS_DELETE,
        permissions::TAGS_READ,
        permissions::TAGS_WRITE,
        permissions::SETTINGS_WHATSAPP_MANAGE,
        permissions::ADMIN_TENANTS_MANAGE,
        permissions::ADMIN_USERS_MANAGE,
    ],
};

pub const TENANT_ADMIN_ROLE: RoleDef = RoleDef {
    name: roles::TENANT_ADMIN,
    description:
        "Tenant admin role with tenant user, settings, contacts, tags, and chat permissions",
    permissions: &[
        permissions::CHATS_READ,
        permissions::CHATS_SEND,
        permissions::CONTACTS_READ,
        permissions::CONTACTS_WRITE,
        permissions::CONTACTS_DELETE,
        permissions::TAGS_READ,
        permissions::TAGS_WRITE,
        permissions::SETTINGS_WHATSAPP_MANAGE,
        permissions::ADMIN_USERS_MANAGE,
    ],
};

pub const AGENT_ROLE: RoleDef = RoleDef {
    name: roles::AGENT,
    description: "Agent role for contacts, tags, and chats",
    permissions: &[
        permissions::CHATS_READ,
        permissions::CHATS_SEND,
        permissions::CONTACTS_READ,
        permissions::CONTACTS_WRITE,
        permissions::TAGS_READ,
        permissions::TAGS_WRITE,
    ],
};

pub const VIEWER_ROLE: RoleDef = RoleDef {
    name: roles::VIEWER,
    description: "Read-only contacts and chats role",
    permissions: &[permissions::CONTACTS_READ, permissions::CHATS_READ],
};

pub fn default_tenant_roles() -> &'static [RoleDef] {
    &[TENANT_ADMIN_ROLE, AGENT_ROLE, VIEWER_ROLE]
}

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

    use super::{default_tenant_roles, permissions, require_permission, roles, PERMISSIONS};
    use crate::auth::context::AuthContext;

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
