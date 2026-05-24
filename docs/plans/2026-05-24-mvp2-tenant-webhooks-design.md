# MVP2: Per-Tenant WhatsApp Webhooks

## Goal

Remove global WhatsApp config. Each tenant owns their WA credentials in DB and gets a unique webhook URL based on their slug.

## Decisions

- **Routing:** Path-based — `/webhook/whatsapp/{tenant_slug}`
- **Global WA config:** Removed entirely from `AppConfig` and `.env`
- **Verify endpoint:** Strict — validates tenant slug + verify_token from DB
- **Webhook URL in settings response:** Yes — computed from `APP_BASE_URL` env var
- **Sender cleanup:** Yes — no more `AppConfig` dependency in sender

## Changes

### 1. Config Cleanup

- Remove from `AppConfig`: `wa_phone_number_id`, `wa_access_token`, `wa_verify_token`, `wa_api_version`
- Add to `AppConfig`: `app_base_url: String` (from `APP_BASE_URL`)
- Update `.env.example`: remove WA vars, add `APP_BASE_URL=http://localhost:8080`
- Fix all test `AppConfig` constructors (remove WA fields)

### 2. Webhook Refactor

**Verify (`GET /webhook/whatsapp/{tenant_slug}`):**
1. Look up tenant by slug → 403 if missing/inactive
2. Look up `tenant_whatsapp_accounts` by tenant_id → 403 if none
3. Compare `hub.verify_token` vs `account.verify_token` → 403 on mismatch
4. Return challenge

**Receive (`POST /webhook/whatsapp/{tenant_slug}`):**
1. Look up tenant by slug → 200 + skip if missing
2. Validate `phone_number_id` from payload matches tenant's account
3. Existing inbound handling unchanged

**Route mount:** `/webhook/whatsapp/{tenant_slug}`

### 3. Sender Cleanup

- `send_whatsapp_message(phone_number_id, access_token, api_version, ...)` — no `AppConfig`
- `upload_media(phone_number_id, access_token, api_version, ...)` — same
- Worker passes tenant account fields directly

### 4. Settings Response

- Add `webhook_url` to `WhatsAppAccountResponse`
- Computed: `{app_base_url}/webhook/whatsapp/{tenant_slug}`
- Requires `AppConfig` + tenant slug lookup in settings handler
