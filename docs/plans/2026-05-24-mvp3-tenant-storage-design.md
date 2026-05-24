# MVP3: Per-Tenant Storage Configuration

## Goal

Each tenant can configure their own R2/S3 storage credentials. If no tenant config exists, system falls back to global storage from `AppConfig`.

## Database

New table `tenant_storage_configs` (1:1 with tenant):

```sql
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

- `region` defaults `auto` (R2 convention); S3 users set real region
- `public_base_url` nullable — for custom domain (e.g. `https://cdn.acme.com`)
- `UNIQUE` on `tenant_id` — one storage config per tenant

## StorageService Changes

Add `for_tenant()` constructor alongside existing `from_config()`:

```rust
pub struct TenantStorageConfig {
    pub endpoint: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
    pub public_base_url: Option<String>,
}

impl StorageService {
    // existing — global fallback from AppConfig
    pub fn from_config(config: &AppConfig) -> Result<Self, String>;

    // new — tenant-specific from DB config
    pub fn for_tenant(config: &TenantStorageConfig) -> Result<Self, String>;
}
```

### Resolution flow (webhook receive + chat upload)

1. Get `tenant_id` from `CurrentUser` or webhook context
2. Query `tenant_storage_configs` WHERE `tenant_id = ? AND is_active = true`
3. If found → `StorageService::for_tenant(config)`
4. If not found → use global `web::Data<StorageService>` fallback

No caching. Media operations are infrequent.

## API Endpoints

Permission: `SETTINGS_STORAGE_MANAGE` (new constant).

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/settings/storage` | Get tenant storage config (404 if none) |
| `POST` | `/api/settings/storage` | Create tenant storage config |
| `PATCH` | `/api/settings/storage` | Update tenant storage config |

1:1 per tenant — no `{id}` path param needed.

### Response shape

```json
{
  "id": 1,
  "tenant_id": 1,
  "endpoint": "https://abc123.r2.cloudflarestorage.com",
  "region": "auto",
  "access_key_id": "AKIAIOSFODNN7EXAMPLE",
  "secret_access_key_masked": "wJal...9kLm",
  "bucket": "acme-crm-media",
  "public_base_url": "https://cdn.acme.com",
  "is_active": true
}
```

- `access_key_id` shown raw (not sensitive, like a username)
- `secret_access_key` masked via `mask_token()` (same as WhatsApp access_token)

### Create/Update request

```json
{
  "endpoint": "https://abc123.r2.cloudflarestorage.com",
  "region": "auto",
  "access_key_id": "AKIAIOSFODNN7EXAMPLE",
  "secret_access_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
  "bucket": "acme-crm-media",
  "public_base_url": "https://cdn.acme.com",
  "is_active": true
}
```

## Worker Impact

Worker (`src/bin/worker.rs`) does NOT use storage — it sends messages via WhatsApp API only. Already tenant-scoped via `outbox_message.tenant_id` + per-tenant WhatsApp account lookup. No changes needed.

## Affected Code

| File | Change |
|------|--------|
| `migrations/006_tenant_storage_config.sql` | New migration |
| `src/models/tenant_storage_config.rs` | New SeaORM entity |
| `src/models/mod.rs` | Register new entity |
| `src/storage/mod.rs` | Add `TenantStorageConfig` struct + `for_tenant()` constructor |
| `src/routes/settings.rs` | Add GET/POST/PATCH `/api/settings/storage` |
| `src/rbac/mod.rs` | Add `SETTINGS_STORAGE_MANAGE` permission |
| `src/whatsapp/webhook.rs` | Resolve tenant storage before media download |
| `src/routes/chat.rs` | Resolve tenant storage before media upload |
| `src/main.rs` | Register new Swagger schemas |
