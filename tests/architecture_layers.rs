use std::path::Path;

#[test]
fn legacy_top_level_modules_are_removed() {
    let forbidden = [
        "src/auth",
        "src/config",
        "src/middleware",
        "src/models",
        "src/rbac",
        "src/storage",
        "src/whatsapp",
        "src/ws",
        "src/common",
        "src/response.rs",
    ];

    for path in forbidden {
        assert!(
            !Path::new(path).exists(),
            "legacy path still exists: {path}"
        );
    }
}

#[test]
fn domain_does_not_import_api_or_transport_frameworks() {
    let forbidden = ["actix_web", "reqwest", "object_store", "crate::api"];

    for entry in walkdir::WalkDir::new("src/domain") {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }

        let content = std::fs::read_to_string(entry.path()).unwrap();
        for needle in forbidden {
            assert!(
                !content.contains(needle),
                "{} contains forbidden import {needle}",
                entry.path().display()
            );
        }
    }
}

#[test]
fn domain_persistence_coupling_is_explicit_and_known() {
    // Temporary guard while persistence mappers still live beside domain entities/repositories.
    // Future cleanup should make this list empty and forbid `sea_orm` + `crate::infrastructure`.
    let allowed = [
        "src/domain/contacts/entities/contact.rs",
        "src/domain/contacts/repositories/mod.rs",
        "src/domain/contacts/repositories/sea_orm_contact_repository.rs",
        "src/domain/messaging/entities/conversation.rs",
        "src/domain/messaging/entities/message.rs",
        "src/domain/messaging/entities/outbox.rs",
        "src/domain/tenants/entities/storage_settings.rs",
        "src/domain/tenants/entities/tenant.rs",
        "src/domain/tenants/entities/whatsapp_settings.rs",
        "src/domain/tenants/repositories/mod.rs",
        "src/domain/tenants/repositories/sea_orm_tenant_repository.rs",
    ];

    let mut offenders = Vec::new();
    for entry in walkdir::WalkDir::new("src/domain") {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }

        let path = entry.path().to_string_lossy().replace('\\', "/");
        let content = std::fs::read_to_string(entry.path()).unwrap();
        if (content.contains("sea_orm") || content.contains("crate::infrastructure"))
            && !allowed.contains(&path.as_str())
        {
            offenders.push(path);
        }
    }

    assert!(
        offenders.is_empty(),
        "new domain persistence coupling found: {offenders:#?}"
    );
}
