# MVP2: Per-Tenant WhatsApp Webhooks — Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Remove global WhatsApp config, make webhook URLs tenant-scoped via slug, return webhook URL in settings API.

**Architecture:** Remove 4 WA fields from `AppConfig`, add `app_base_url`. Webhook routes become `/webhook/whatsapp/{tenant_slug}`. Sender/media functions take explicit params instead of `AppConfig`. Settings response includes computed `webhook_url`.

**Tech Stack:** Actix-web, SeaORM, existing WhatsApp modules.

---

### Task 1: Remove global WA config from AppConfig

**Files:**
- Modify: `src/config/mod.rs`
- Modify: `.env.example`

**Step 1: Update `AppConfig` struct and `from_env()`**

In `src/config/mod.rs`, remove these 4 fields and their `from_env()` lines:
- `wa_phone_number_id`
- `wa_access_token`
- `wa_verify_token`
- `wa_api_version`

Add new field:
```rust
pub app_base_url: String,
```

In `from_env()`, add:
```rust
app_base_url: std::env::var("APP_BASE_URL")
    .unwrap_or_else(|_| "http://localhost:8080".to_string()),
```

**Step 2: Update `.env.example`**

Remove:
```
WA_PHONE_NUMBER_ID=your-phone-number-id
WA_BUSINESS_ACCOUNT_ID=your-business-account-id
WA_API_VERSION=v25.0
WA_VERIFY_TOKEN=change-me
WA_ACCESS_TOKEN=
```

Add:
```
APP_BASE_URL=http://localhost:8080
```

**Step 3: Fix all test `AppConfig` constructors**

In every test that builds `AppConfig`, remove the 4 WA fields and add `app_base_url`. Files:
- `src/auth/extractor.rs` (~line 139-142) — remove 4 WA lines, add `app_base_url: "http://localhost:8080".into()`
- `src/routes/admin.rs` (~lines 844-847, 902-905, 957-960) — same in all 3 test helpers
- `src/routes/auth.rs` (~lines 141-144) — same
- `src/routes/contacts.rs` (~lines 712-715) — same
- `src/routes/settings.rs` (~lines 373-376) — same
- `src/storage/mod.rs` (~lines 163-166) — same
- `src/ws/mod.rs` (~lines 119-122) — same

**Step 4: Verify compilation**

Run: `cargo check`
Expected: Compiles (webhook.rs/sender.rs/media.rs will have errors — fixed in later tasks)

Actually, webhook/sender/media still reference `config.wa_*` — need to stub those out temporarily OR do Task 2+3 first. Better approach: do all in one commit.

**Revised Step 4: Stub webhook/sender/media references**

In `src/whatsapp/webhook.rs` line 29, change:
```rust
// OLD:
if mode == "subscribe" && token == config.wa_verify_token {
// NEW (temporary — replaced in Task 2):
if mode == "subscribe" {
    // TODO: verify against tenant account
```

In `src/whatsapp/sender.rs`, remove `WhatsAppSender::new()` method entirely (only `from_parts()` remains). Remove `use crate::config::AppConfig;`.

In `src/whatsapp/media.rs`, remove `get_media_url()` and `download_media_binary()` wrappers (the `_with_token` versions are the ones actually used). Remove `use crate::config::AppConfig;`. Remove `download_and_save()` (legacy PoC, not used in MVP1).

**Step 5: Run checks**

Run: `cargo check`
Expected: Clean compile

Run: `cargo test --lib -- --test-threads=1`
Expected: All pass

**Step 6: Commit**

```bash
git add -A
git commit -m "refactor: remove global WhatsApp config from AppConfig"
```

---

### Task 2: Refactor webhook to tenant-slug routing

**Files:**
- Modify: `src/whatsapp/webhook.rs`

**Step 1: Add tenant_slug path parameter to verify**

Replace the `verify` handler:

```rust
#[derive(serde::Deserialize)]
pub struct WebhookPath {
    pub tenant_slug: String,
}

/// Webhook verification — Meta sends GET to verify
#[get("/{tenant_slug}")]
pub async fn verify(
    path: web::Path<WebhookPath>,
    query: web::Query<VerifyQuery>,
    db: web::Data<DatabaseConnection>,
) -> HttpResponse {
    let mode = query.mode.as_deref().unwrap_or("");
    let token = query.verify_token.as_deref().unwrap_or("");
    let challenge = query.challenge.as_deref().unwrap_or("");

    if mode != "subscribe" {
        return HttpResponse::Forbidden().finish();
    }

    // Look up tenant by slug
    let tenant = match crate::models::tenant::Entity::find()
        .filter(crate::models::tenant::Column::Slug.eq(&path.tenant_slug))
        .filter(crate::models::tenant::Column::IsActive.eq(true))
        .one(db.get_ref())
        .await
    {
        Ok(Some(t)) => t,
        _ => {
            tracing::warn!(slug = %path.tenant_slug, "Webhook verify: tenant not found");
            return HttpResponse::Forbidden().finish();
        }
    };

    // Look up tenant WhatsApp account
    let account = match tenant_whatsapp_account::Entity::find()
        .filter(tenant_whatsapp_account::Column::TenantId.eq(tenant.id))
        .filter(tenant_whatsapp_account::Column::IsActive.eq(true))
        .one(db.get_ref())
        .await
    {
        Ok(Some(a)) => a,
        _ => {
            tracing::warn!(tenant_id = tenant.id, "Webhook verify: no WA account");
            return HttpResponse::Forbidden().finish();
        }
    };

    if token != account.verify_token {
        tracing::warn!(slug = %path.tenant_slug, "Webhook verify: token mismatch");
        return HttpResponse::Forbidden().finish();
    }

    tracing::info!(slug = %path.tenant_slug, "Webhook verified");
    HttpResponse::Ok().body(challenge.to_string())
}
```

**Step 2: Add tenant_slug path parameter to receive**

Update `receive` handler signature:

```rust
#[post("/{tenant_slug}")]
pub async fn receive(
    path: web::Path<WebhookPath>,
    body: web::Json<WebhookPayload>,
    db: web::Data<DatabaseConnection>,
    storage: web::Data<StorageService>,
    hub: web::Data<actix::Addr<ChatHub>>,
) -> HttpResponse {
    // Look up tenant by slug — return 200 to Meta even if not found
    let tenant = match crate::models::tenant::Entity::find()
        .filter(crate::models::tenant::Column::Slug.eq(&path.tenant_slug))
        .filter(crate::models::tenant::Column::IsActive.eq(true))
        .one(db.get_ref())
        .await
    {
        Ok(Some(t)) => t,
        _ => {
            tracing::warn!(slug = %path.tenant_slug, "Webhook receive: tenant not found, skipping");
            return HttpResponse::Ok().finish();
        }
    };

    for entry in &body.entry {
        // ... existing entry/change loop, but add tenant_id cross-check:
        // after resolve_whatsapp_account, verify account.tenant_id == tenant.id
```

In the existing `resolve_whatsapp_account` result handling, add after `Ok(Some(account))`:
```rust
if account.tenant_id != tenant.id {
    tracing::warn!(
        slug = %path.tenant_slug,
        account_tenant = account.tenant_id,
        "Webhook phone_number_id does not match tenant slug"
    );
    continue;
}
```

**Step 3: Update route scope**

Change `configure()`:
```rust
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/webhook/whatsapp")
            .service(verify)
            .service(receive),
    );
}
```

No change needed — `/{tenant_slug}` is already inside the scope.

**Step 4: Remove `config: web::Data<AppConfig>` from verify handler** (already done above — verify no longer uses AppConfig)

**Step 5: Run checks**

Run: `cargo check`
Expected: Clean compile

Run: `cargo test --lib -- --test-threads=1`
Expected: All pass

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: per-tenant webhook routing via slug"
```

---

### Task 3: Clean up sender and media modules

**Files:**
- Modify: `src/whatsapp/sender.rs`
- Modify: `src/whatsapp/media.rs`

**Step 1: Clean sender.rs**

- Remove `use crate::config::AppConfig;`
- Remove `WhatsAppSender::new(config: &AppConfig)` method (if not already done in Task 1)
- Fix `upload_media()` — it uses `std::env::var("WA_API_VERSION")` and `WA_PHONE_NUMBER_ID`. Change to use `self.base_url` parts or add `api_version`/`phone_number_id` fields to `WhatsAppSender`:

```rust
pub struct WhatsAppSender {
    client: Client,
    base_url: String,      // messages URL
    media_url: String,     // media upload URL
    access_token: String,
}

impl WhatsAppSender {
    pub fn from_parts(api_version: &str, phone_number_id: &str, access_token: &str) -> Self {
        let base_url = format!(
            "https://graph.facebook.com/{}/{}/messages",
            api_version, phone_number_id
        );
        let media_url = format!(
            "https://graph.facebook.com/{}/{}/media",
            api_version, phone_number_id
        );
        Self {
            client: Client::new(),
            base_url,
            media_url,
            access_token: access_token.to_string(),
        }
    }
```

Update `upload_media()` to use `self.media_url` instead of building URL from env vars:
```rust
pub async fn upload_media(&self, file_path: &str, mime_type: &str) -> Result<String, String> {
    // Use self.media_url instead of env vars
    let resp = self.client
        .post(&self.media_url)
        .bearer_auth(&self.access_token)
        // ... rest unchanged
```

**Step 2: Clean media.rs**

- Remove `use crate::config::AppConfig;`
- Remove `get_media_url()` (wrapper around `_with_token`)
- Remove `download_media_binary()` (wrapper around `_with_token`)
- Remove `download_and_save()` (PoC legacy, unused in MVP1)
- Keep: `get_media_url_with_token()`, `download_media_binary_with_token()`, `download_bytes()`, `mime_to_extension()`
- Make `get_media_url_with_token` and `download_media_binary_with_token` `pub` (if not already)

**Step 3: Run checks**

Run: `cargo check`
Expected: Clean

Run: `cargo test --lib -- --test-threads=1`
Expected: All pass

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor: remove global config from sender and media modules"
```

---

### Task 4: Add webhook_url to settings response

**Files:**
- Modify: `src/routes/settings.rs`

**Step 1: Add webhook_url field to response**

Add to `WhatsAppAccountResponse`:
```rust
pub webhook_url: String,
```

**Step 2: Compute webhook_url in handlers**

In the settings handlers that return `WhatsAppAccountResponse`, look up tenant slug and compute URL.

Add helper:
```rust
async fn build_account_response(
    account: &tenant_whatsapp_account::Model,
    tenant_slug: &str,
    app_base_url: &str,
) -> WhatsAppAccountResponse {
    let masked = if account.access_token.len() > 8 {
        format!("{}...{}", &account.access_token[..4], &account.access_token[account.access_token.len()-4..])
    } else {
        "****".to_string()
    };
    WhatsAppAccountResponse {
        id: account.id,
        tenant_id: account.tenant_id,
        phone_number_id: account.phone_number_id.clone(),
        business_account_id: account.business_account_id.clone(),
        display_phone_number: account.display_phone_number.clone(),
        access_token_masked: masked,
        verify_token: account.verify_token.clone(),
        api_version: account.api_version.clone(),
        is_active: account.is_active,
        webhook_url: format!("{}/webhook/whatsapp/{}", app_base_url.trim_end_matches('/'), tenant_slug),
    }
}
```

**Step 3: Inject `AppConfig` and look up tenant slug in GET/POST/PATCH handlers**

Each handler needs:
- `config: web::Data<AppConfig>` parameter
- Tenant slug lookup: `tenant::Entity::find_by_id(ctx.tenant_id).one(db).await` to get slug
- Call `build_account_response(&account, &tenant.slug, &config.app_base_url)`

**Step 4: Run checks**

Run: `cargo check`
Expected: Clean

Run: `cargo test --lib -- --test-threads=1`
Expected: All pass

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: return webhook_url in WhatsApp settings response"
```

---

### Task 5: Update Swagger docs & final verification

**Files:**
- Modify: `src/main.rs` (if Swagger paths need update)

**Step 1: Verify Swagger reflects new webhook paths**

The utoipa `#[get("/{tenant_slug}")]` and `#[post("/{tenant_slug}")]` attributes under `/webhook/whatsapp` scope should auto-generate correct paths. Check if `src/main.rs` openapi paths list needs updating for the new path format.

**Step 2: Update `.env` (local only, not committed)**

In your local `.env`, remove old WA vars if still present, add:
```
APP_BASE_URL=http://localhost:8080
```

**Step 3: Full verification**

Run:
```bash
cargo check
cargo test --lib -- --test-threads=1
cargo fmt --check
cargo clippy -- -D warnings 2>&1 | head -30
```

Expected: All clean.

**Step 4: Commit & push**

```bash
git add -A
git commit -m "docs: update swagger for MVP2 tenant webhooks"
git push origin master
```
