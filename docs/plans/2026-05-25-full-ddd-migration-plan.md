# Full DDD Migration Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Complete migration from pragmatic DDD to a stricter DDD/layered architecture by moving legacy top-level business-ish modules into `domain`, `api`, or `infrastructure` and removing compatibility shims.

**Architecture:** Keep domain pure where practical: entities, value objects, factories, repositories, services, and domain errors live under `src/domain`. Put framework/database/WhatsApp/R2/WebSocket implementations under `src/infrastructure`. Keep HTTP-only DTOs/routes under `src/api`. SeaORM models remain persistence records, moved under infrastructure only after imports are stable.

**Tech Stack:** Rust, Actix-web, SeaORM, MySQL/MariaDB, JWT, Argon2, Cloudflare R2/S3, Meta WhatsApp Cloud API, WebSocket actors, utoipa.

---

## Current State

Current post-MVP4 structure has DDD domains but still has top-level legacy modules:

- `src/auth/` — JWT, password, extractor/context
- `src/config/` — `AppConfig`
- `src/middleware/` — token validation
- `src/models/` — SeaORM persistence models
- `src/rbac/` — permission constants/helpers
- `src/response.rs` — HTTP response helpers
- `src/storage/` — local/R2 storage service implementation
- `src/whatsapp/` — Meta sender, webhook, media downloader
- `src/ws/` — WebSocket hub/session

Target high-level shape:

```text
src/
├── api/
│   ├── dto/
│   ├── middleware/
│   ├── responses.rs
│   └── routes/
├── application/
│   ├── auth/
│   ├── contacts/
│   ├── messaging/
│   ├── tenants/
│   └── storage/
├── domain/
│   ├── auth/
│   ├── contacts/
│   ├── messaging/
│   ├── storage/
│   └── tenants/
├── infrastructure/
│   ├── config/
│   ├── persistence/
│   │   ├── models/
│   │   └── sea_orm/
│   ├── security/
│   ├── storage/
│   ├── whatsapp/
│   └── websocket/
├── bin/
├── lib.rs
└── main.rs
```

Notes:
- `application/` is orchestration layer for use cases that need multiple domain services/adapters.
- `domain/` should not depend on Actix, SeaORM, object-store, reqwest, or WebSocket actors.
- `api/` may depend on Actix/utoipa and application services.
- `infrastructure/` may depend on SeaORM, reqwest, object-store, JWT libs, etc.

---

## Task 1: Add `application` and `infrastructure` Skeleton

**Files:**
- Create: `src/application/mod.rs`
- Create: `src/application/auth/mod.rs`
- Create: `src/application/contacts/mod.rs`
- Create: `src/application/messaging/mod.rs`
- Create: `src/application/storage/mod.rs`
- Create: `src/application/tenants/mod.rs`
- Create: `src/infrastructure/mod.rs`
- Create: `src/infrastructure/config/mod.rs`
- Create: `src/infrastructure/persistence/mod.rs`
- Create: `src/infrastructure/security/mod.rs`
- Create: `src/infrastructure/storage/mod.rs`
- Create: `src/infrastructure/whatsapp/mod.rs`
- Create: `src/infrastructure/websocket/mod.rs`
- Modify: `src/lib.rs`

**Step 1: Write failing compile test**

Create `tests/layer_exports.rs`:

```rust
#[test]
fn new_layers_are_exported() {
    let _ = std::any::type_name::<crm_bro::application::auth::Marker>();
    let _ = std::any::type_name::<crm_bro::infrastructure::config::Marker>();
}
```

**Step 2: Run test to verify fail**

Run:

```bash
cargo test --test layer_exports
```

Expected: fail because `application` and `infrastructure` missing.

**Step 3: Add modules and temporary marker types**

Each new `mod.rs` gets:

```rust
pub struct Marker;
```

`src/application/mod.rs`:

```rust
pub mod auth;
pub mod contacts;
pub mod messaging;
pub mod storage;
pub mod tenants;
```

`src/infrastructure/mod.rs`:

```rust
pub mod config;
pub mod persistence;
pub mod security;
pub mod storage;
pub mod whatsapp;
pub mod websocket;
```

`src/lib.rs` add:

```rust
pub mod application;
pub mod infrastructure;
```

**Step 4: Verify**

Run:

```bash
cargo test --test layer_exports
cargo fmt --check
```

Expected: pass.

**Step 5: Commit**

```bash
git add src/application src/infrastructure src/lib.rs tests/layer_exports.rs
git commit -m "refactor: add application and infrastructure layers"
```

---

## Task 2: Move Config to Infrastructure

**Files:**
- Move: `src/config/mod.rs` → `src/infrastructure/config/app_config.rs`
- Modify: `src/infrastructure/config/mod.rs`
- Modify: `src/common/config.rs`
- Modify imports across `src/` and `tests/`
- Modify: `src/lib.rs`

**Goal:** `AppConfig` becomes infrastructure configuration, not top-level module.

**Step 1: Write failing import test**

Modify `tests/layer_exports.rs`:

```rust
#[test]
fn app_config_lives_in_infrastructure() {
    let _ = crm_bro::infrastructure::config::AppConfig::from_env;
}
```

Run:

```bash
cargo test --test layer_exports app_config_lives_in_infrastructure
```

Expected: fail until moved/exported.

**Step 2: Move file**

```bash
mkdir -p src/infrastructure/config
mv src/config/mod.rs src/infrastructure/config/app_config.rs
```

`src/infrastructure/config/mod.rs`:

```rust
pub mod app_config;
pub use app_config::AppConfig;
```

**Step 3: Update imports**

Replace:

```rust
use crate::config::AppConfig;
use crm_bro::config::AppConfig;
```

With:

```rust
use crate::infrastructure::config::AppConfig;
use crm_bro::infrastructure::config::AppConfig;
```

For compatibility during migration, `src/common/config.rs` may keep:

```rust
pub use crate::infrastructure::config::*;
```

Remove `pub mod config;` from `src/lib.rs` after imports are fixed.

**Step 4: Verify**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: move app config to infrastructure"
```

---

## Task 3: Move HTTP Response Helpers to API

**Files:**
- Move: `src/response.rs` → `src/api/responses.rs`
- Modify: `src/api/mod.rs`
- Modify: imports in routes and tests
- Modify: `src/common/error.rs`
- Modify: `src/lib.rs`

**Goal:** Response helpers are API concern, not domain/common concern.

**Step 1: Test API response export**

Add to `tests/layer_exports.rs`:

```rust
#[test]
fn response_helpers_live_in_api() {
    let _ = crm_bro::api::responses::ok::<()>;
}
```

Run expected fail.

**Step 2: Move and export**

```bash
mv src/response.rs src/api/responses.rs
```

`src/api/mod.rs` add:

```rust
pub mod responses;
```

Update imports:

```rust
crate::response::
```

to:

```rust
crate::api::responses::
```

Update `src/common/error.rs`:

```rust
pub use crate::api::responses::*;
```

Remove `pub mod response;` from `src/lib.rs` after direct imports gone.

**Step 3: Verify**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor: move response helpers to api layer"
```

---

## Task 4: Move Middleware to API Middleware

**Files:**
- Move: `src/middleware/mod.rs` → `src/api/middleware/mod.rs`
- Modify: `src/api/mod.rs`
- Modify imports
- Modify: `src/common/middleware.rs`
- Modify: `src/lib.rs`

**Goal:** Actix middleware belongs to API layer.

**Step 1: Test export**

Add:

```rust
#[test]
fn auth_middleware_lives_in_api() {
    let _ = crm_bro::api::middleware::validate_token;
}
```

Run expected fail.

**Step 2: Move and update**

```bash
mkdir -p src/api/middleware
mv src/middleware/mod.rs src/api/middleware/mod.rs
```

`src/api/mod.rs` add:

```rust
pub mod middleware;
```

Update imports:

```rust
crate::middleware::
```

to:

```rust
crate::api::middleware::
```

Update `src/common/middleware.rs`:

```rust
pub use crate::api::middleware::*;
```

Remove `pub mod middleware;` from `src/lib.rs` when direct imports gone.

**Step 3: Verify**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor: move middleware to api layer"
```

---

## Task 5: Split Auth into Domain, Application, Infrastructure, API

**Files:**
- Move security code:
  - `src/auth/jwt.rs` → `src/infrastructure/security/jwt.rs`
  - `src/auth/password.rs` → `src/infrastructure/security/password.rs`
- Move request context/extractor:
  - `src/auth/context.rs` → `src/api/middleware/auth_context.rs` or `src/api/auth/context.rs`
  - `src/auth/extractor.rs` → `src/api/middleware/auth_extractor.rs`
- Modify: `src/domain/auth/`
- Modify: `src/application/auth/mod.rs`
- Modify imports
- Remove: `src/auth/`
- Modify: `src/lib.rs`

**Goal:** Domain auth has business concepts; infrastructure has cryptography/JWT; API has extractors.

**Step 1: Add tests for new exports**

`tests/layer_exports.rs`:

```rust
#[test]
fn security_lives_in_infrastructure() {
    let _ = crm_bro::infrastructure::security::hash_password;
    let _ = crm_bro::infrastructure::security::encode_jwt;
}

#[test]
fn current_user_lives_in_api() {
    let _ = std::any::type_name::<crm_bro::api::middleware::CurrentUser>();
}
```

Expected fail.

**Step 2: Move JWT/password**

`src/infrastructure/security/mod.rs`:

```rust
pub mod jwt;
pub mod password;

pub use jwt::*;
pub use password::*;
```

Move files and update imports.

**Step 3: Move extractor/context**

`src/api/middleware/mod.rs` should export:

```rust
pub mod auth_context;
pub mod auth_extractor;

pub use auth_context::*;
pub use auth_extractor::*;
```

Move `CurrentUser`, auth extractor logic, and update routes:

```rust
use crate::api::middleware::CurrentUser;
```

**Step 4: Update domain auth**

Ensure `src/domain/auth/entities/user.rs` does not depend on JWT/Actix.

**Step 5: Remove top-level auth module**

After all imports changed:

```bash
rm -rf src/auth
```

Remove `pub mod auth;` from `src/lib.rs`.

Update `src/common/auth.rs` during transition:

```rust
pub use crate::api::middleware::{AuthContext, CurrentUser};
pub use crate::infrastructure::security::{build_claims, decode_jwt, encode_jwt, hash_password, verify_password};
```

Delete `src/common/auth.rs` in final cleanup task if no imports use it.

**Step 6: Verify**

```bash
cargo fmt --check
cargo test auth
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 7: Commit**

```bash
git add -A
git commit -m "refactor: split auth across api domain and infrastructure"
```

---

## Task 6: Move RBAC into Domain Auth/Application Auth

**Files:**
- Move: `src/rbac/mod.rs` → split into:
  - `src/domain/auth/permissions.rs`
  - `src/application/auth/rbac.rs`
- Modify imports in admin/settings/routes
- Remove: `src/rbac/`
- Modify: `src/lib.rs`

**Goal:** Permission names are auth domain concepts; DB checks/use-case enforcement are application/API concerns.

**Step 1: Add tests**

```rust
#[test]
fn permissions_live_in_domain_auth() {
    assert_eq!(crm_bro::domain::auth::permissions::CONTACTS_READ, "contacts:read");
}
```

Run expected fail.

**Step 2: Move constants**

Create `src/domain/auth/permissions.rs` with existing constants.

`src/domain/auth/mod.rs` add:

```rust
pub mod permissions;
```

**Step 3: Move helper logic**

If `src/rbac/mod.rs` contains DB/helper functions, move them to `src/application/auth/rbac.rs` because they are use-case/persistence aware.

`src/application/auth/mod.rs`:

```rust
pub mod rbac;
pub use rbac::*;
```

**Step 4: Update imports**

Replace:

```rust
crate::rbac::permissions
```

With:

```rust
crate::domain::auth::permissions
```

Replace helper imports with:

```rust
crate::application::auth::...
```

**Step 5: Remove top-level `rbac`**

```bash
rm -rf src/rbac
```

Remove `pub mod rbac;` from `src/lib.rs`.

**Step 6: Verify**

```bash
cargo fmt --check
cargo test rbac
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 7: Commit**

```bash
git add -A
git commit -m "refactor: move rbac into auth domain"
```

---

## Task 7: Move Storage Runtime to Infrastructure + Domain Port

**Files:**
- Move: `src/storage/mod.rs` → `src/infrastructure/storage/object_storage.rs`
- Modify: `src/infrastructure/storage/mod.rs`
- Create: `src/domain/storage/repositories/object_store.rs` or `src/domain/storage/services/storage_port.rs`
- Modify imports in routes/webhook/chat
- Remove: top-level `src/storage/`
- Modify: `src/lib.rs`

**Goal:** Domain defines storage config/port; infrastructure implements local/R2 storage.

**Step 1: Add storage port trait test**

Add domain port:

```rust
#[async_trait::async_trait]
pub trait ObjectStorage: Send + Sync {
    async fn put(&self, key: &str, bytes: Vec<u8>, content_type: &str) -> Result<StoredObject, StorageError>;
}
```

Write compile test for trait export.

**Step 2: Move implementation**

```bash
mkdir -p src/infrastructure/storage
mv src/storage/mod.rs src/infrastructure/storage/object_storage.rs
```

`src/infrastructure/storage/mod.rs`:

```rust
pub mod object_storage;
pub use object_storage::*;
```

**Step 3: Update imports**

Replace:

```rust
crate::storage::StorageService
```

With:

```rust
crate::infrastructure::storage::StorageService
```

**Step 4: Remove top-level module**

Remove `pub mod storage;` from `src/lib.rs` after direct references gone.

**Step 5: Verify**

```bash
cargo fmt --check
cargo test storage
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 6: Commit**

```bash
git add -A
git commit -m "refactor: move storage runtime to infrastructure"
```

---

## Task 8: Move WhatsApp Client/Media to Infrastructure + Messaging Ports

**Files:**
- Move:
  - `src/whatsapp/sender.rs` → `src/infrastructure/whatsapp/sender.rs`
  - `src/whatsapp/media.rs` → `src/infrastructure/whatsapp/media.rs`
  - `src/whatsapp/types.rs` → `src/infrastructure/whatsapp/types.rs`
- Keep webhook route as API:
  - `src/whatsapp/webhook.rs` → `src/api/routes/webhook.rs` or `src/api/routes/whatsapp_webhook.rs`
- Modify: `src/api/routes/mod.rs`
- Modify: `src/main.rs`
- Create domain/application ports for WhatsApp sending if needed
- Remove: `src/whatsapp/`
- Modify: `src/lib.rs`

**Goal:** Meta API details are infrastructure; webhook handler is API; message decisions live in application/domain.

**Step 1: Add infrastructure export test**

```rust
#[test]
fn whatsapp_sender_lives_in_infrastructure() {
    let _ = std::any::type_name::<crm_bro::infrastructure::whatsapp::WhatsAppSender>();
}
```

Expected fail.

**Step 2: Move sender/media/types**

```bash
mkdir -p src/infrastructure/whatsapp
mv src/whatsapp/sender.rs src/infrastructure/whatsapp/sender.rs
mv src/whatsapp/media.rs src/infrastructure/whatsapp/media.rs
mv src/whatsapp/types.rs src/infrastructure/whatsapp/types.rs
```

`src/infrastructure/whatsapp/mod.rs`:

```rust
pub mod media;
pub mod sender;
pub mod types;

pub use sender::WhatsAppSender;
```

**Step 3: Move webhook route**

```bash
mv src/whatsapp/webhook.rs src/api/routes/whatsapp_webhook.rs
```

Update `src/api/routes/mod.rs`:

```rust
pub mod whatsapp_webhook;
```

Update main/service configuration so webhook route stays outside `/api` if current behavior requires:

```rust
.service(web::scope("/webhook/whatsapp").configure(routes::whatsapp_webhook::configure))
```

**Step 4: Update imports**

Replace:

```rust
crate::whatsapp::sender
crate::whatsapp::media
crate::whatsapp::types
```

With:

```rust
crate::infrastructure::whatsapp::sender
crate::infrastructure::whatsapp::media
crate::infrastructure::whatsapp::types
```

**Step 5: Remove top-level module**

```bash
rm -rf src/whatsapp
```

Remove `pub mod whatsapp;` from `src/lib.rs`.

**Step 6: Verify webhook path still documented**

Run unit/integration tests and, if local DB available, manually verify route registration with existing webhook tests.

```bash
cargo fmt --check
cargo test webhook
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 7: Commit**

```bash
git add -A
git commit -m "refactor: move whatsapp integration to infrastructure"
```

---

## Task 9: Move WebSocket to Infrastructure/API Realtime Adapter

**Files:**
- Move:
  - `src/ws/hub.rs` → `src/infrastructure/websocket/hub.rs`
  - `src/ws/session.rs` → `src/infrastructure/websocket/session.rs`
- Modify: `src/infrastructure/websocket/mod.rs`
- Create: `src/api/routes/websocket.rs` if route setup currently in `main.rs`
- Remove: `src/ws/`
- Modify imports
- Modify: `src/lib.rs`

**Goal:** WebSocket actors are infrastructure delivery mechanism; API owns route registration.

**Step 1: Add export test**

```rust
#[test]
fn websocket_hub_lives_in_infrastructure() {
    let _ = std::any::type_name::<crm_bro::infrastructure::websocket::ChatHub>();
}
```

Expected fail.

**Step 2: Move modules**

```bash
mkdir -p src/infrastructure/websocket
mv src/ws/hub.rs src/infrastructure/websocket/hub.rs
mv src/ws/session.rs src/infrastructure/websocket/session.rs
```

`src/infrastructure/websocket/mod.rs`:

```rust
pub mod hub;
pub mod session;

pub use hub::*;
pub use session::*;
```

**Step 3: Update imports**

Replace:

```rust
crate::ws::
```

With:

```rust
crate::infrastructure::websocket::
```

**Step 4: Remove top-level `ws`**

```bash
rm -rf src/ws
```

Remove `pub mod ws;` from `src/lib.rs`.

**Step 5: Verify**

```bash
cargo fmt --check
cargo test ws
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 6: Commit**

```bash
git add -A
git commit -m "refactor: move websocket runtime to infrastructure"
```

---

## Task 10: Move SeaORM Models Under Infrastructure Persistence

**Files:**
- Move: `src/models/` → `src/infrastructure/persistence/models/`
- Modify: `src/infrastructure/persistence/mod.rs`
- Modify imports across project
- Modify: `src/lib.rs`

**Goal:** Persistence records are infrastructure, not domain.

**Step 1: Add export test**

```rust
#[test]
fn seaorm_models_live_under_infrastructure() {
    let _ = std::any::type_name::<crm_bro::infrastructure::persistence::models::tenant::Model>();
}
```

Expected fail.

**Step 2: Move directory**

```bash
mkdir -p src/infrastructure/persistence
mv src/models src/infrastructure/persistence/models
```

`src/infrastructure/persistence/mod.rs`:

```rust
pub mod models;
```

**Step 3: Update imports**

Replace:

```rust
crate::models::
crm_bro::models::
```

With:

```rust
crate::infrastructure::persistence::models::
crm_bro::infrastructure::persistence::models::
```

For SeaORM generated module self-references, adjust `super::` imports only if compile fails.

**Step 4: Remove top-level `models`**

Remove `pub mod models;` from `src/lib.rs`.

**Step 5: Verify carefully**

```bash
cargo check
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 6: Commit**

```bash
git add -A
git commit -m "refactor: move persistence models to infrastructure"
```

---

## Task 11: Replace Common Re-exports with Direct Layer Imports

**Files:**
- Remove: `src/common/auth.rs`
- Remove: `src/common/config.rs`
- Remove: `src/common/error.rs`
- Remove: `src/common/middleware.rs`
- Remove: `src/common/mod.rs`
- Modify imports across project
- Modify: `src/lib.rs`

**Goal:** `common` was MVP4 bridge; full DDD should use explicit layer imports.

**Step 1: Find common imports**

Run:

```bash
grep -R "crate::common\|crm_bro::common" -n src tests
```

Expected: list imports to replace.

**Step 2: Replace imports**

Use direct paths:

- Auth security: `crate::infrastructure::security::*`
- Current user/extractor: `crate::api::middleware::*`
- Config: `crate::infrastructure::config::AppConfig`
- Responses: `crate::api::responses::*`

**Step 3: Remove common module**

```bash
rm -rf src/common
```

Remove `pub mod common;` from `src/lib.rs`.

**Step 4: Verify**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: remove common compatibility layer"
```

---

## Task 12: Move Use-Case Logic from Routes into Application Services

**Files:**
- Create/modify:
  - `src/application/contacts/contact_use_cases.rs`
  - `src/application/messaging/chat_use_cases.rs`
  - `src/application/messaging/webhook_use_cases.rs`
  - `src/application/tenants/settings_use_cases.rs`
- Modify:
  - `src/api/routes/contacts.rs`
  - `src/api/routes/chat.rs`
  - `src/api/routes/settings.rs`
  - `src/api/routes/whatsapp_webhook.rs`

**Goal:** Routes should parse auth/path/body, call application service, map result to HTTP.

**Step 1: Pick one route group first: Contacts**

Add application service function for contacts list filters:

```rust
pub struct ListContactsInput {
    pub tenant_id: i32,
    pub q: Option<String>,
    pub tag: Option<String>,
    pub owner_user_id: Option<i32>,
    pub page: u64,
    pub per_page: u64,
}
```

Move SeaORM query logic from route into service.

**Step 2: Test application service logic**

Reuse current contact integration test or extract new service-level test.

Run:

```bash
cargo test contacts
```

**Step 3: Thin contacts route**

Route should:

1. Get `tenant_id`
2. Build input
3. Call service
4. Return JSON

**Step 4: Repeat for chat/settings/webhook**

Order:
1. Contacts
2. Settings
3. Chat send/list/search
4. Webhook receive/status

Each group gets separate commit.

**Step 5: Verify after each group**

```bash
cargo fmt --check
cargo test <group>
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 6: Commits**

```bash
git commit -m "refactor: move contacts use cases to application layer"
git commit -m "refactor: move settings use cases to application layer"
git commit -m "refactor: move chat use cases to application layer"
git commit -m "refactor: move webhook use cases to application layer"
```

---

## Task 13: Final Architecture Guard Tests

**Files:**
- Create: `tests/architecture_layers.rs`

**Goal:** Prevent reintroducing old top-level modules and forbidden dependencies.

**Step 1: Add directory absence tests**

```rust
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
        assert!(!std::path::Path::new(path).exists(), "legacy path still exists: {path}");
    }
}
```

**Step 2: Add domain forbidden import check**

```rust
#[test]
fn domain_does_not_import_framework_or_infrastructure() {
    let forbidden = ["actix_web", "sea_orm", "reqwest", "object_store", "crate::infrastructure", "crate::api"];
    for entry in walkdir::WalkDir::new("src/domain") {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let content = std::fs::read_to_string(entry.path()).unwrap();
        for needle in forbidden {
            assert!(!content.contains(needle), "{} contains forbidden import {needle}", entry.path().display());
        }
    }
}
```

Add `walkdir` dev-dependency if not present:

```toml
[dev-dependencies]
walkdir = "2"
```

Note: if domain repository traits currently mention SeaORM model conversions, refactor those conversions into infrastructure before enabling this test.

**Step 3: Verify**

```bash
cargo test --test architecture_layers
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock tests/architecture_layers.rs
git commit -m "test: enforce architecture layer boundaries"
```

---

## Task 14: Documentation + Final Verification

**Files:**
- Modify: `README.md`
- Create: `docs/architecture/ddd-layers.md`
- Modify: existing plan docs if needed

**Goal:** Document final architecture and migration rules.

**Step 1: Update README architecture tree**

README should show only final target modules.

**Step 2: Add DDD layer docs**

`docs/architecture/ddd-layers.md`:

```markdown
# DDD Layer Guide

## Domain
Pure business rules. No Actix, SeaORM, reqwest, object_store.

## Application
Use cases and orchestration. May depend on domain traits and infrastructure adapters through constructor injection.

## API
HTTP/WebSocket route glue. Parse request, call application service, return response.

## Infrastructure
Database, external APIs, storage, JWT/password hashing, WebSocket runtime.
```

**Step 3: Final verification**

Run:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Expected:
- all pass
- architecture tests pass

**Step 4: Commit**

```bash
git add README.md docs/architecture/ddd-layers.md
git commit -m "docs: document final DDD architecture"
```

---

## Task 15: Release Preparation

**Files:**
- No code changes expected.

**Step 1: Confirm clean repo**

```bash
git status --short
```

Expected: no output.

**Step 2: Push branch**

```bash
git push origin master
```

**Step 3: Tag release**

Use next alpha unless user requests stable:

```bash
git tag -a v0.1.0-alpha.4 -m "Full DDD migration"
git push origin v0.1.0-alpha.4
```

Expected image:

```text
ghcr.io/igun997/crm-bro:0.1.0-alpha.4
```

---

## Risk Notes

1. Moving `src/models/` is highest-risk because SeaORM generated imports touch many files. Do this after auth/storage/whatsapp/ws moves are stable.
2. Moving webhook and worker logic into application services can regress real WhatsApp behavior. Keep current integration tests and verify manually after release.
3. Strict domain purity may require moving `from_model`/`to_active_model` methods out of domain entities into infrastructure mappers.
4. Do not combine multiple module moves in one commit. One layer/concern per commit.
5. Keep `cargo clippy --all-targets --all-features -- -D warnings` passing after every task.

---

## Suggested Execution Mode

Use subagent-driven development, one task per subagent, with code review after each task. For Task 10 and Task 12, prefer `9router/cx/gpt-5.5` or stronger because imports and behavior risk are high.
