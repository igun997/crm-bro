# MVP4 DDD Refactor Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Refactor CRM-Bro from flat layered structure into pragmatic DDD modules with domain entities, factories, services, repositories, and thin HTTP routes.

**Architecture:** Incremental domain-by-domain migration. Keep existing SeaORM models in `src/models` as persistence records while introducing domain entities under `src/domain`. Move shared infrastructure to `src/common`, HTTP DTO/routes to `src/api`, and keep behavior compatible after each task.

**Tech Stack:** Rust, Actix-web, SeaORM, MySQL/MariaDB, utoipa, async-trait, thiserror, mockall.

---

## Preconditions

- Current branch: `master`
- No worktrees per project preference.
- Use TDD for feature/bugfix work.
- Run verification before claiming complete.
- Design doc: `docs/plans/2026-05-25-mvp4-ddd-refactor-design.md`

## Verification Commands

Run after each task when touched code compiles:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

For DB-dependent tests, ensure local DB env exists:

```bash
export DATABASE_URL='mysql://crmbro_user_ca8149:3fe8Z15!d1@localhost/crmbro'
```

---

## Task 1: Add DDD Dependencies and Skeleton Modules

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Create: `src/common/mod.rs`
- Create: `src/domain/mod.rs`
- Create: `src/api/mod.rs`
- Create: `src/domain/storage/mod.rs`
- Create: `src/domain/auth/mod.rs`
- Create: `src/domain/tenants/mod.rs`
- Create: `src/domain/contacts/mod.rs`
- Create: `src/domain/messaging/mod.rs`

**Step 1: Add dependencies**

Add to `Cargo.toml`:

```toml
async-trait = "0.1"
thiserror = "1.0"
```

Add to `[dev-dependencies]`:

```toml
mockall = "0.11"
```

If `thiserror` already exists, skip duplicate.

**Step 2: Add module declarations**

In `src/lib.rs`, expose:

```rust
pub mod api;
pub mod common;
pub mod domain;
```

Keep existing modules exported until migrated.

**Step 3: Create empty module files**

`src/domain/mod.rs`:

```rust
pub mod auth;
pub mod contacts;
pub mod messaging;
pub mod storage;
pub mod tenants;
```

`src/common/mod.rs`:

```rust
pub mod auth;
pub mod config;
pub mod error;
pub mod middleware;
```

`src/api/mod.rs`:

```rust
pub mod dto;
pub mod routes;
```

Domain `mod.rs` files start empty or with placeholder comments.

**Step 4: Verify**

Run:

```bash
cargo fmt --check
cargo test
```

Expected: all tests pass.

**Step 5: Commit**

```bash
git add Cargo.toml src/lib.rs src/common src/domain src/api
git commit -m "refactor: add DDD module skeleton"
```

---

## Task 2: Move Shared Infrastructure to `common` with Re-exports

**Files:**
- Create: `src/common/auth/mod.rs`
- Create: `src/common/config/mod.rs`
- Create: `src/common/error/mod.rs`
- Create: `src/common/middleware/mod.rs`
- Modify: `src/lib.rs`
- Keep old paths alive initially: `src/auth/*`, `src/config/mod.rs`, `src/middleware/mod.rs`, `src/response.rs`

**Step 1: Create re-export modules**

`src/common/auth/mod.rs`:

```rust
pub use crate::auth::*;
```

`src/common/config/mod.rs`:

```rust
pub use crate::config::*;
```

`src/common/middleware/mod.rs`:

```rust
pub use crate::middleware::*;
```

`src/common/error/mod.rs`:

```rust
pub use crate::response::*;
```

**Step 2: No behavior changes**

This task only creates stable new import paths. Do not update call sites yet.

**Step 3: Verify**

```bash
cargo fmt --check
cargo test
```

**Step 4: Commit**

```bash
git add src/common
git commit -m "refactor: expose shared infrastructure under common"
```

---

## Task 3: Refactor Storage Domain First

**Files:**
- Create: `src/domain/storage/errors.rs`
- Create: `src/domain/storage/services.rs`
- Modify: `src/domain/storage/mod.rs`
- Modify: `src/storage/mod.rs`

**Step 1: Write failing tests**

Add tests in `src/domain/storage/services.rs` for config resolution behavior:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_storage_config_requires_directory() {
        let result = StorageConfigFactory::local("");
        assert!(matches!(result, Err(StorageError::InvalidConfig(_))));
    }

    #[test]
    fn r2_storage_config_requires_bucket() {
        let result = StorageConfigFactory::r2("endpoint", "", "key", "secret", None);
        assert!(matches!(result, Err(StorageError::InvalidConfig(_))));
    }
}
```

Expected fail: `StorageConfigFactory` not found.

**Step 2: Implement domain error**

`src/domain/storage/errors.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Invalid storage config: {0}")]
    InvalidConfig(String),
    #[error("Storage operation failed: {0}")]
    Operation(String),
    #[error("Database error: {0}")]
    Database(String),
}
```

**Step 3: Implement storage config factory**

`src/domain/storage/services.rs`:

```rust
use super::errors::StorageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageBackendKind {
    Local,
    R2,
    S3,
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub backend: StorageBackendKind,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub region: Option<String>,
    pub public_base_url: Option<String>,
    pub local_dir: Option<String>,
}

pub struct StorageConfigFactory;

impl StorageConfigFactory {
    pub fn local(local_dir: impl Into<String>) -> Result<StorageConfig, StorageError> {
        let local_dir = local_dir.into();
        if local_dir.trim().is_empty() {
            return Err(StorageError::InvalidConfig("local_dir is required".into()));
        }
        Ok(StorageConfig {
            backend: StorageBackendKind::Local,
            endpoint: None,
            bucket: None,
            access_key_id: None,
            secret_access_key: None,
            region: None,
            public_base_url: None,
            local_dir: Some(local_dir),
        })
    }

    pub fn r2(
        endpoint: impl Into<String>,
        bucket: impl Into<String>,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
        public_base_url: Option<String>,
    ) -> Result<StorageConfig, StorageError> {
        Self::object_storage(
            StorageBackendKind::R2,
            endpoint,
            bucket,
            access_key_id,
            secret_access_key,
            Some("auto".to_string()),
            public_base_url,
        )
    }

    pub fn object_storage(
        backend: StorageBackendKind,
        endpoint: impl Into<String>,
        bucket: impl Into<String>,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
        region: Option<String>,
        public_base_url: Option<String>,
    ) -> Result<StorageConfig, StorageError> {
        let endpoint = endpoint.into();
        let bucket = bucket.into();
        let access_key_id = access_key_id.into();
        let secret_access_key = secret_access_key.into();

        if endpoint.trim().is_empty() {
            return Err(StorageError::InvalidConfig("endpoint is required".into()));
        }
        if bucket.trim().is_empty() {
            return Err(StorageError::InvalidConfig("bucket is required".into()));
        }
        if access_key_id.trim().is_empty() {
            return Err(StorageError::InvalidConfig("access_key_id is required".into()));
        }
        if secret_access_key.trim().is_empty() {
            return Err(StorageError::InvalidConfig("secret_access_key is required".into()));
        }

        Ok(StorageConfig {
            backend,
            endpoint: Some(endpoint),
            bucket: Some(bucket),
            access_key_id: Some(access_key_id),
            secret_access_key: Some(secret_access_key),
            region,
            public_base_url,
            local_dir: None,
        })
    }
}
```

**Step 4: Export storage domain**

`src/domain/storage/mod.rs`:

```rust
pub mod errors;
pub mod services;

pub use errors::StorageError;
pub use services::{StorageBackendKind, StorageConfig, StorageConfigFactory};
```

**Step 5: Verify**

```bash
cargo test domain::storage
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 6: Commit**

```bash
git add src/domain/storage src/storage/mod.rs
git commit -m "refactor: introduce storage domain factory"
```

---

## Task 4: Auth Domain Entities and Factories

**Files:**
- Create: `src/domain/auth/entities/mod.rs`
- Create: `src/domain/auth/entities/user.rs`
- Create: `src/domain/auth/errors.rs`
- Modify: `src/domain/auth/mod.rs`

**Step 1: Write failing tests**

`src/domain/auth/entities/user.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_user_rejects_empty_email() {
        let result = User::new(1, "".into(), "Name".into(), "hash".into());
        assert!(matches!(result, Err(AuthError::InvalidEmail(_))));
    }

    #[test]
    fn new_user_normalizes_email() {
        let user = User::new(1, "ADMIN@EXAMPLE.COM".into(), "Admin".into(), "hash".into()).unwrap();
        assert_eq!(user.email(), "admin@example.com");
    }
}
```

**Step 2: Implement AuthError**

`src/domain/auth/errors.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid email: {0}")]
    InvalidEmail(String),
    #[error("Invalid name: {0}")]
    InvalidName(String),
    #[error("Invalid password hash")]
    InvalidPasswordHash,
    #[error("User not found")]
    UserNotFound,
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Database error: {0}")]
    Database(String),
}
```

**Step 3: Implement User entity factory**

`src/domain/auth/entities/user.rs`:

```rust
use crate::domain::auth::errors::AuthError;

#[derive(Debug, Clone)]
pub struct User {
    id: i32,
    tenant_id: Option<i32>,
    email: String,
    name: String,
    password_hash: String,
    is_active: bool,
}

impl User {
    pub fn new(
        tenant_id: i32,
        email: String,
        name: String,
        password_hash: String,
    ) -> Result<Self, AuthError> {
        Self::new_with_optional_tenant(Some(tenant_id), email, name, password_hash)
    }

    pub fn new_superadmin(
        email: String,
        name: String,
        password_hash: String,
    ) -> Result<Self, AuthError> {
        Self::new_with_optional_tenant(None, email, name, password_hash)
    }

    fn new_with_optional_tenant(
        tenant_id: Option<i32>,
        email: String,
        name: String,
        password_hash: String,
    ) -> Result<Self, AuthError> {
        let email = email.trim().to_lowercase();
        let name = name.trim().to_string();

        if !email.contains('@') || email.len() < 3 {
            return Err(AuthError::InvalidEmail(email));
        }
        if name.is_empty() {
            return Err(AuthError::InvalidName("name cannot be empty".into()));
        }
        if password_hash.trim().is_empty() {
            return Err(AuthError::InvalidPasswordHash);
        }

        Ok(Self {
            id: 0,
            tenant_id,
            email,
            name,
            password_hash,
            is_active: true,
        })
    }

    pub fn id(&self) -> i32 { self.id }
    pub fn tenant_id(&self) -> Option<i32> { self.tenant_id }
    pub fn email(&self) -> &str { &self.email }
    pub fn name(&self) -> &str { &self.name }
    pub fn password_hash(&self) -> &str { &self.password_hash }
    pub fn is_active(&self) -> bool { self.is_active }
}
```

**Step 4: Export modules**

`src/domain/auth/entities/mod.rs`:

```rust
pub mod user;
pub use user::User;
```

`src/domain/auth/mod.rs`:

```rust
pub mod entities;
pub mod errors;

pub use entities::User;
pub use errors::AuthError;
```

**Step 5: Verify and commit**

```bash
cargo test domain::auth
cargo fmt --check
git add src/domain/auth
git commit -m "refactor: introduce auth domain user factory"
```

---

## Task 5: Contacts Domain Entities, Repository, Service

**Files:**
- Create: `src/domain/contacts/entities/mod.rs`
- Create: `src/domain/contacts/entities/contact.rs`
- Create: `src/domain/contacts/errors.rs`
- Create: `src/domain/contacts/repositories/mod.rs`
- Create: `src/domain/contacts/repositories/contact_repository.rs`
- Create: `src/domain/contacts/services/mod.rs`
- Create: `src/domain/contacts/services/contact_service.rs`
- Modify: `src/domain/contacts/mod.rs`

**Step 1: Write entity tests**

Test invalid phone, empty name, normalization.

**Step 2: Implement `ContactError`**

Include `NotFound`, `InvalidName`, `InvalidPhone`, `DuplicatePhone`, `Database`.

**Step 3: Implement `Contact` entity factory**

Fields mirror `src/models/contact.rs`; keep getters.

**Step 4: Implement `ContactRepository` trait**

Trait only. SeaORM impl can be added in next task.

**Step 5: Implement `ContactService<R: ContactRepository>`**

Methods: `create`, `get`, `list`, `delete`.

**Step 6: Add mock-based service tests**

Use `mockall` to verify:
- duplicate phone returns `ContactError::DuplicatePhone`
- invalid phone does not call save
- valid contact calls save

**Step 7: Verify and commit**

```bash
cargo test domain::contacts
cargo fmt --check
git add src/domain/contacts
git commit -m "refactor: introduce contacts domain service"
```

---

## Task 6: Contacts SeaORM Repository Adapter

**Files:**
- Create: `src/domain/contacts/repositories/sea_orm_contact_repository.rs`
- Modify: `src/domain/contacts/repositories/mod.rs`
- Modify: `src/domain/contacts/entities/contact.rs`
- Test: existing contact tests

**Step 1: Add reconstitution method**

In `Contact`, add:

```rust
pub(crate) fn from_model(model: crate::models::contact::Model) -> Self { ... }
pub(crate) fn to_active_model(&self) -> crate::models::contact::ActiveModel { ... }
```

**Step 2: Implement SeaORM repository**

Implement `ContactRepository` using existing `crate::models::contact` entity.

**Step 3: Run integration tests**

```bash
cargo test contacts
```

**Step 4: Commit**

```bash
git add src/domain/contacts
git commit -m "refactor: add contacts SeaORM repository"
```

---

## Task 7: Move Contacts HTTP DTOs to `api/dto`

**Files:**
- Create: `src/api/dto/mod.rs`
- Create: `src/api/dto/contacts.rs`
- Modify: `src/routes/contacts.rs`
- Modify: `src/api/mod.rs`

**Step 1: Move request/response structs**

Move `CreateContactRequest`, `UpdateContactRequest`, response structs from route file to DTO file. Keep utoipa derives and examples.

**Step 2: Add `From<Contact>` conversions**

```rust
impl From<crate::domain::contacts::Contact> for ContactResponse { ... }
```

**Step 3: Verify Swagger compile**

```bash
cargo check
cargo test
```

**Step 4: Commit**

```bash
git add src/api/dto src/routes/contacts.rs
git commit -m "refactor: move contact DTOs to api layer"
```

---

## Task 8: Thin Contacts Routes via Domain Service

**Files:**
- Modify: `src/routes/contacts.rs`
- Modify: `src/api/dto/contacts.rs`

**Step 1: Update handlers**

Replace direct SeaORM calls in contact route handlers with:

```rust
let repo = SeaOrmContactRepository::new(db.get_ref().clone());
let service = ContactService::new(repo);
```

Then call domain service.

**Step 2: Preserve API behavior**

Response JSON must match current API shape. Swagger unchanged.

**Step 3: Run tests**

```bash
cargo test contacts
cargo test
```

**Step 4: Commit**

```bash
git add src/routes/contacts.rs src/api/dto/contacts.rs
git commit -m "refactor: route contacts through domain service"
```

---

## Task 9: Tenants Domain

**Files:**
- Create: `src/domain/tenants/entities/*`
- Create: `src/domain/tenants/errors.rs`
- Create: `src/domain/tenants/repositories/*`
- Create: `src/domain/tenants/services/*`
- Modify: `src/routes/settings.rs`

**Scope:**
- Tenant entity factory validates name/slug.
- WhatsApp settings factory validates phone number ID, business account ID, access token, verify token.
- Storage settings factory uses storage domain config factory.
- Repository wraps `tenant`, `tenant_whatsapp_account`, `tenant_storage_config` SeaORM models.
- Settings routes become thin.

**Verification:**

```bash
cargo test domain::tenants
cargo test settings
cargo test
```

**Commit:**

```bash
git add src/domain/tenants src/routes/settings.rs
git commit -m "refactor: introduce tenants domain services"
```

---

## Task 10: Messaging Domain

**Files:**
- Create: `src/domain/messaging/entities/*`
- Create: `src/domain/messaging/errors.rs`
- Create: `src/domain/messaging/repositories/*`
- Create: `src/domain/messaging/services/*`
- Modify: `src/routes/chat.rs`
- Modify: `src/whatsapp/webhook.rs`
- Modify: `src/bin/worker.rs`

**Scope:**
- Conversation factory enforces tenant/contact/account invariants.
- Message factory supports inbound/outbound text/template/media.
- Outbox factory enforces queued status and payload shape.
- ChatService handles outbound message creation + outbox enqueue.
- WebhookService handles inbound message persistence + media storage dispatch.
- OutboxService handles claim/send/update flow.

**Known bug to fix here:** Outbound message can end with `wa_message_id` set but `status='failed'`. Add test for success-after-retry or status update precedence: if Meta send returns success, final message status must be `sent`.

**Verification:**

```bash
cargo test domain::messaging
cargo test chat
cargo test webhook
cargo test
```

**Commit:**

```bash
git add src/domain/messaging src/routes/chat.rs src/whatsapp/webhook.rs src/bin/worker.rs
git commit -m "refactor: introduce messaging domain services"
```

---

## Task 11: Move Routes to `api/routes`

**Files:**
- Create: `src/api/routes/mod.rs`
- Move/copy: `src/routes/*.rs` -> `src/api/routes/*.rs`
- Modify: `src/main.rs`
- Modify: `src/lib.rs`
- Keep `src/routes/mod.rs` as re-export temporarily

**Step 1: Create route re-exports**

`src/api/routes/mod.rs` exports route modules.

**Step 2: Update main imports**

Change route imports from `crate::routes` to `crate::api::routes`.

**Step 3: Keep backward compatibility**

`src/routes/mod.rs` can re-export `crate::api::routes::*` for now.

**Step 4: Verify and commit**

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
git add src/api/routes src/routes src/main.rs src/lib.rs
git commit -m "refactor: move HTTP routes under api layer"
```

---

## Task 12: Cleanup Old Paths and Final Verification

**Files:**
- Modify: `src/lib.rs`
- Remove or re-export old modules only if no longer used.
- Update docs if paths changed.

**Step 1: Search old direct imports**

```bash
grep -R "crate::routes\|crate::storage\|crate::auth\|crate::config\|crate::middleware" -n src
```

Move imports to `crate::api`, `crate::domain`, `crate::common` where appropriate.

**Step 2: Run full verification**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: pass.

**Step 3: Optional smoke test**

Start API locally and verify:

```bash
cargo run --bin crm-bro
curl http://127.0.0.1:8080/api/health
```

Expected:

```json
{"status":"ok","database":"connected"}
```

**Step 4: Commit**

```bash
git add src docs/plans/2026-05-25-mvp4-ddd-refactor-plan.md
git commit -m "refactor: complete MVP4 DDD structure cleanup"
```

---

## Done Criteria

- `cargo fmt --check` passes.
- `cargo test` passes.
- `cargo clippy --all-targets --all-features -- -D warnings` passes.
- HTTP API behavior unchanged.
- Swagger still builds.
- Domain factories validate invariants.
- Routes are thin and call services.
- SeaORM models remain persistence-only.
- Known outbound status bug fixed in messaging domain.
