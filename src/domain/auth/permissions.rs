pub const CHATS_READ: &str = "chats.read";
pub const CHATS_SEND: &str = "chats.send";
pub const CONTACTS_READ: &str = "contacts.read";
pub const CONTACTS_WRITE: &str = "contacts.write";
pub const CONTACTS_DELETE: &str = "contacts.delete";
pub const TAGS_READ: &str = "tags.read";
pub const TAGS_WRITE: &str = "tags.write";
pub const SETTINGS_WHATSAPP_MANAGE: &str = "settings.whatsapp.manage";
pub const SETTINGS_STORAGE_MANAGE: &str = "settings.storage.manage";
pub const ADMIN_TENANTS_MANAGE: &str = "admin.tenants.manage";
pub const ADMIN_USERS_MANAGE: &str = "admin.users.manage";

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
        code: CHATS_READ,
        description: "Read conversations and messages",
    },
    PermissionDef {
        code: CHATS_SEND,
        description: "Send messages",
    },
    PermissionDef {
        code: CONTACTS_READ,
        description: "Read contacts",
    },
    PermissionDef {
        code: CONTACTS_WRITE,
        description: "Create and update contacts",
    },
    PermissionDef {
        code: CONTACTS_DELETE,
        description: "Delete contacts",
    },
    PermissionDef {
        code: TAGS_READ,
        description: "Read tags",
    },
    PermissionDef {
        code: TAGS_WRITE,
        description: "Create and update tags",
    },
    PermissionDef {
        code: SETTINGS_WHATSAPP_MANAGE,
        description: "Manage WhatsApp account settings",
    },
    PermissionDef {
        code: SETTINGS_STORAGE_MANAGE,
        description: "Manage tenant storage configuration",
    },
    PermissionDef {
        code: ADMIN_TENANTS_MANAGE,
        description: "Create and manage tenants",
    },
    PermissionDef {
        code: ADMIN_USERS_MANAGE,
        description: "Create and manage users",
    },
];

pub const SUPERADMIN_ROLE: RoleDef = RoleDef {
    name: roles::SUPERADMIN,
    description: "System superadmin role with all permissions",
    permissions: &[
        CHATS_READ,
        CHATS_SEND,
        CONTACTS_READ,
        CONTACTS_WRITE,
        CONTACTS_DELETE,
        TAGS_READ,
        TAGS_WRITE,
        SETTINGS_WHATSAPP_MANAGE,
        SETTINGS_STORAGE_MANAGE,
        ADMIN_TENANTS_MANAGE,
        ADMIN_USERS_MANAGE,
    ],
};

pub const TENANT_ADMIN_ROLE: RoleDef = RoleDef {
    name: roles::TENANT_ADMIN,
    description:
        "Tenant admin role with tenant user, settings, contacts, tags, and chat permissions",
    permissions: &[
        CHATS_READ,
        CHATS_SEND,
        CONTACTS_READ,
        CONTACTS_WRITE,
        CONTACTS_DELETE,
        TAGS_READ,
        TAGS_WRITE,
        SETTINGS_WHATSAPP_MANAGE,
        SETTINGS_STORAGE_MANAGE,
        ADMIN_USERS_MANAGE,
    ],
};

pub const AGENT_ROLE: RoleDef = RoleDef {
    name: roles::AGENT,
    description: "Agent role for contacts, tags, and chats",
    permissions: &[
        CHATS_READ,
        CHATS_SEND,
        CONTACTS_READ,
        CONTACTS_WRITE,
        TAGS_READ,
        TAGS_WRITE,
    ],
};

pub const VIEWER_ROLE: RoleDef = RoleDef {
    name: roles::VIEWER,
    description: "Read-only contacts and chats role",
    permissions: &[CONTACTS_READ, CHATS_READ],
};

pub fn default_tenant_roles() -> &'static [RoleDef] {
    &[TENANT_ADMIN_ROLE, AGENT_ROLE, VIEWER_ROLE]
}
