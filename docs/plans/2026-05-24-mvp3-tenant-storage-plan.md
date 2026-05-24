# MVP3: Per-Tenant Storage Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Each tenant configures their own R2/S3 storage; fallback to global storage when no tenant config exists.

**Architecture:** New `tenant_storage_configs` table (1:1 per tenant). `StorageService::for_tenant()` constructor builds tenant-specific instance. Webhook receive + chat upload resolve tenant storage from DB, fall back to global `web::Data<StorageService>`.

**Tech Stack:** Rust, Actix-web, SeaORM, object_store crate, utoipa (Swagger)

---

### Task 1: Migration + SeaORM Model

**Files:**
- Create: `migrations/006_tenant_storage_config.sql`
- Create: `src/models/tenant_storage_config.rs`
- Modify: `src/models/mod.rs`

**Step 1: Create migration**

```sql
-- migrations/006_tenant_storage_config.sql
CREATE TABLE IF NOT EXISTS tenant_storage_configs (
    id                INT AUTO_INCREMENT PRIMARY KEY,
    tenant_id         INT NOT NULL UNIQUE,
    endpoint          VARCHAR(512) NOT NULL,
    region            VARCHAR(64) NOT NULL DEFAULT 'auto',
    access_key_id     VARCHAR(256) NOT NULL,
    secret_access_key VARCHAR(512) NOT NULL,
    bucket            VARCHAR(256) NOT NULL,
    public_base_url   VARCHAR(512) DEFAULT NULL,
    is_active         TINYINT(1) NOT NULL DEFAULT 1,
    created_at        DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at        DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id)
);
```

**Step 2: Create SeaORM entity**

```rust
// src/models/tenant_storage_config.rs
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tenant_storage_configs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub tenant_id: i32,
    pub endpoint: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
    pub public_base_url: Option<String>,
    pub is_active: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tenant::Entity",
        from = "Column::TenantId",
        to = "super::tenant::Column::Id"
    )]
    Tenant,
}

impl ActiveModelBehavior for ActiveModel {}
```

**Step 3: Register in `src/models/mod.rs`**

Add `pub mod tenant_storage_config;` alongside existing model modules.

**Step 4: Run migration locally**

```bash
make migrate
```

**Step 5: Verify compile**

```bash
cargo check
```

**Step 6: Commit**

```bash
git add -A && git commit -m "feat: add tenant_storage_configs migration and SeaORM model"
```

---

### Task 2: StorageService `for_tenant()` Constructor

**Files:**
- Modify: `src/storage/mod.rs`

**Step 1: Add `TenantStorageConfig` struct and `for_tenant()` constructor**

Add after existing `from_config()`:

```rust
/// Lightweight struct for building tenant-specific storage.
/// Populated from `tenant_storage_configs` DB row.
#[derive(Debug, Clone)]
pub struct TenantStorageConfig {
    pub endpoint: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
    pub public_base_url: Option<String>,
}

impl StorageService {
    /// Build tenant-specific R2/S3 storage from DB config.
    pub fn for_tenant(config: &TenantStorageConfig) -> Result<Self, String> {
        let store = AmazonS3Builder::new()
            .with_endpoint(&config.endpoint)
            .with_region(&config.region)
            .with_access_key_id(&config.access_key_id)
            .with_secret_access_key(&config.secret_access_key)
            .with_bucket_name(&config.bucket)
            .build()
            .map_err(|error| format!("Failed to build tenant storage: {error}"))?;
        Ok(Self {
            backend: StorageBackend::R2 {
                store: Arc::new(store),
                public_base_url: config.public_base_url.clone(),
            },
        })
    }
}
```

**Step 2: Add helper to resolve tenant storage with global fallback**

```rust
use crate::models::tenant_storage_config;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

impl StorageService {
    /// Resolve storage for a tenant: use tenant config if exists + active, else return None (caller uses global).
    pub async fn resolve_for_tenant(
        db: &DatabaseConnection,
        tenant_id: i32,
    ) -> Result<Option<Self>, String> {
        let config = tenant_storage_config::Entity::find()
            .filter(tenant_storage_config::Column::TenantId.eq(tenant_id))
            .filter(tenant_storage_config::Column::IsActive.eq(true))
            .one(db)
            .await
            .map_err(|error| format!("DB lookup tenant storage config: {error}"))?;

        match config {
            Some(row) => {
                let tenant_config = TenantStorageConfig {
                    endpoint: row.endpoint,
                    region: row.region,
                    access_key_id: row.access_key_id,
                    secret_access_key: row.secret_access_key,
                    bucket: row.bucket,
                    public_base_url: row.public_base_url,
                };
                Ok(Some(Self::for_tenant(&tenant_config)?))
            }
            None => Ok(None),
        }
    }
}
```

**Step 3: Add tests**

```rust
#[test]
fn for_tenant_builds_r2_backend() {
    let config = TenantStorageConfig {
        endpoint: "https://abc.r2.cloudflarestorage.com".to_string(),
        region: "auto".to_string(),
        access_key_id: "AKID".to_string(),
        secret_access_key: "SECRET".to_string(),
        bucket: "test-bucket".to_string(),
        public_base_url: Some("https://cdn.example.com".to_string()),
    };
    let service = StorageService::for_tenant(&config).unwrap();
    // Verify URL generation uses public_base_url
    let rt = tokio::runtime::Runtime::new().unwrap();
    let url = rt.block_on(service.get_url("test/file.jpg")).unwrap();
    assert_eq!(url, "https://cdn.example.com/test/file.jpg");
}

#[test]
fn for_tenant_without_public_url_returns_key() {
    let config = TenantStorageConfig {
        endpoint: "https://abc.r2.cloudflarestorage.com".to_string(),
        region: "auto".to_string(),
        access_key_id: "AKID".to_string(),
        secret_access_key: "SECRET".to_string(),
        bucket: "test-bucket".to_string(),
        public_base_url: None,
    };
    let service = StorageService::for_tenant(&config).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let url = rt.block_on(service.get_url("test/file.jpg")).unwrap();
    assert_eq!(url, "test/file.jpg");
}
```

**Step 4: Run tests**

```bash
cargo test --lib storage -- --test-threads=1
```

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: add StorageService::for_tenant() with DB resolution and fallback"
```

---

### Task 3: RBAC Permission + Settings API Endpoints

**Files:**
- Modify: `src/rbac/mod.rs` — add `SETTINGS_STORAGE_MANAGE` permission constant
- Modify: `src/routes/settings.rs` — add GET/POST/PATCH `/api/settings/storage`
- Modify: `src/main.rs` — register new Swagger schemas

**Step 1: Add permission constant**

In `src/rbac/mod.rs`, add to the `permissions` module:

```rust
pub const SETTINGS_STORAGE_MANAGE: &str = "settings:storage:manage";
```

Also add to `TENANT_ADMIN` role's permissions list in `default_tenant_roles()`.

**Step 2: Add request/response structs in `src/routes/settings.rs`**

```rust
#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "id": 1, "tenant_id": 1,
    "endpoint": "https://abc123.r2.cloudflarestorage.com",
    "region": "auto",
    "access_key_id": "AKIAIOSFODNN7EXAMPLE",
    "secret_access_key_masked": "wJal...9kLm",
    "bucket": "acme-crm-media",
    "public_base_url": "https://cdn.acme.com",
    "is_active": true
}))]
pub struct StorageConfigResponse {
    pub id: i32,
    pub tenant_id: i32,
    pub endpoint: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key_masked: String,
    pub bucket: String,
    pub public_base_url: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({
    "endpoint": "https://abc123.r2.cloudflarestorage.com",
    "region": "auto",
    "access_key_id": "AKIAIOSFODNN7EXAMPLE",
    "secret_access_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
    "bucket": "acme-crm-media",
    "public_base_url": "https://cdn.acme.com"
}))]
pub struct CreateStorageConfigRequest {
    pub endpoint: String,
    pub region: Option<String>,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
    pub public_base_url: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"bucket": "new-bucket", "is_active": true}))]
pub struct PatchStorageConfigRequest {
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub bucket: Option<String>,
    pub public_base_url: Option<String>,
    pub is_active: Option<bool>,
}
```

**Step 3: Implement handlers**

- `GET /api/settings/storage` — lookup by `ctx.tenant_id`, return 404 if none, mask `secret_access_key` via `mask_token()`
- `POST /api/settings/storage` — insert new row, `region` defaults to `"auto"`, return conflict if exists
- `PATCH /api/settings/storage` — find existing by `tenant_id`, apply partial updates, mask secret in response

All require `SETTINGS_STORAGE_MANAGE` permission + tenant context.

Build response helper:

```rust
fn build_storage_config_response(row: &tenant_storage_config::Model) -> StorageConfigResponse {
    StorageConfigResponse {
        id: row.id,
        tenant_id: row.tenant_id,
        endpoint: row.endpoint.clone(),
        region: row.region.clone(),
        access_key_id: row.access_key_id.clone(),
        secret_access_key_masked: mask_token(&row.secret_access_key),
        bucket: row.bucket.clone(),
        public_base_url: row.public_base_url.clone(),
        is_active: row.is_active,
    }
}
```

**Step 4: Register in `configure()` and add utoipa paths**

Add `#[utoipa::path(...)]` annotations with tag `"Settings"`. Register services in `settings::configure()`.

**Step 5: Update `src/main.rs` Swagger schema list**

Add `StorageConfigResponse`, `CreateStorageConfigRequest`, `PatchStorageConfigRequest` to `#[openapi(components(schemas(...)))]`.

**Step 6: Run `cargo check` + tests**

```bash
cargo check && cargo test --lib -- --test-threads=1
```

**Step 7: Commit**

```bash
git add -A && git commit -m "feat: add tenant storage config settings API (GET/POST/PATCH)"
```

---

### Task 4: Wire Tenant Storage into Webhook Receive

**Files:**
- Modify: `src/whatsapp/webhook.rs`

**Step 1: Update `receive()` handler**

After resolving tenant + account, resolve tenant storage:

```rust
// In receive(), after tenant is resolved:
let tenant_storage = match StorageService::resolve_for_tenant(db.get_ref(), tenant.id).await {
    Ok(ts) => ts,
    Err(error) => {
        tracing::error!(%error, "Failed to resolve tenant storage");
        None
    }
};
let effective_storage = tenant_storage.as_ref().unwrap_or(storage.get_ref());
```

Then pass `effective_storage` instead of `storage.get_ref()` to `handle_inbound_message()` and downstream `store_message_media()`.

**Step 2: Verify compile + existing tests pass**

```bash
cargo check && cargo test --lib -- --test-threads=1
```

**Step 3: Commit**

```bash
git add -A && git commit -m "feat: resolve tenant storage in webhook receive with global fallback"
```

---

### Task 5: Wire Tenant Storage into Chat Upload

**Files:**
- Modify: `src/routes/chat.rs`

**Step 1: Update `send_upload()` handler**

After getting `CurrentUser` with `tenant_id`, resolve tenant storage:

```rust
let tenant_id = match ctx.tenant_id {
    Some(id) => id,
    None => return HttpResponse::Forbidden().json(...),
};

let tenant_storage = match StorageService::resolve_for_tenant(db.get_ref(), tenant_id).await {
    Ok(ts) => ts,
    Err(error) => {
        tracing::error!(%error, "Failed to resolve tenant storage");
        None
    }
};
let effective_storage = tenant_storage.as_ref().unwrap_or(storage.get_ref());
```

Use `effective_storage` for `storage.put(...)` call instead of global `storage`.

**Step 2: Verify compile + existing tests pass**

```bash
cargo check && cargo test --lib -- --test-threads=1
```

**Step 3: Commit**

```bash
git add -A && git commit -m "feat: resolve tenant storage in chat upload with global fallback"
```

---

### Task 6: Final Verification + Push

**Files:**
- None (verification only)

**Step 1: cargo fmt**

```bash
cargo fmt
```

**Step 2: cargo clippy**

```bash
cargo clippy --all-targets -- -D warnings
```

**Step 3: Full test suite**

```bash
cargo test --lib -- --test-threads=1
```

**Step 4: Commit + push**

```bash
git add -A && git commit -m "chore: cargo fmt + final MVP3 verification"
git push origin master
```
