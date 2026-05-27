# CRM-Bro

[![CI](https://github.com/igun997/crm-bro/actions/workflows/ci.yml/badge.svg)](https://github.com/igun997/crm-bro/actions/workflows/ci.yml)

Multi-tenant WhatsApp CRM built with Rust.

## Docker Deployment (Recommended)

### Quick Start with Docker Compose

```bash
# 1. Create env file
cat > .env <<'EOF'
DB_USER=crmbro
DB_PASSWORD=your-db-password
DB_NAME=crmbro
JWT_SECRET=your-jwt-secret-change-me
APP_BASE_URL=https://crm.yourdomain.com
STORAGE_BACKEND=local

# First deploy only — remove after initial setup
ADMIN_EMAIL=admin@yourdomain.com
ADMIN_PASSWORD=your-secure-admin-password
ADMIN_NAME=Admin
EOF

# 2. Start everything
docker compose up -d

# 3. Check logs
docker compose logs -f api
```

On first start the entrypoint will:
1. Wait for MariaDB to be ready
2. Run all SQL migrations
3. Seed the superadmin user (if `ADMIN_EMAIL` + `ADMIN_PASSWORD` are set)
4. Start the API server

> ⚠️ **Remove `ADMIN_EMAIL` / `ADMIN_PASSWORD` from `.env` after first deploy** — they're only needed for initial setup.

### Available Commands

The Docker image supports multiple commands:

| Command | Description |
|---------|-------------|
| `api` (default) | Run migrations + seed admin + start API server |
| `worker` | Start the outbox message worker |
| `seed` | Run migrations + seed admin only (no server) |
| `migrate` | Run migrations only |

```bash
# Run worker separately
docker run --env-file .env ghcr.io/igun997/crm-bro:latest worker

# Seed admin manually
docker run --env-file .env \
  -e ADMIN_EMAIL=admin@example.com \
  -e ADMIN_PASSWORD=changeme \
  ghcr.io/igun997/crm-bro:latest seed
```

### Docker Image Tags

| Tag | Description |
|-----|-------------|
| `latest` | Latest stable release |
| `0.1.0` | Specific version |
| `0.1` | Latest patch in minor |
| `0.1.0-alpha.3` | Pre-release (not tagged as `latest`) |

### Production with R2/S3 Storage

```bash
# Add to .env
STORAGE_BACKEND=r2
R2_ENDPOINT=https://your-account.r2.cloudflarestorage.com
R2_ACCESS_KEY_ID=your-key
R2_SECRET_ACCESS_KEY=your-secret
R2_BUCKET=crm-media
R2_PUBLIC_BASE_URL=https://cdn.yourdomain.com
```

### Environment Variables Reference

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | Yes | — | MySQL/MariaDB connection string |
| `JWT_SECRET` | Yes | — | JWT signing secret |
| `APP_BASE_URL` | Yes | `http://localhost:8080` | Public URL of the API |
| `STORAGE_BACKEND` | No | `local` | `local` or `r2` |
| `STORAGE_LOCAL_DIR` | No | `media` | Local media directory |
| `R2_ENDPOINT` | If R2 | — | R2/S3 endpoint URL |
| `R2_ACCESS_KEY_ID` | If R2 | — | R2/S3 access key |
| `R2_SECRET_ACCESS_KEY` | If R2 | — | R2/S3 secret key |
| `R2_BUCKET` | If R2 | — | R2/S3 bucket name |
| `R2_PUBLIC_BASE_URL` | No | — | CDN URL for media |
| `ADMIN_EMAIL` | No | — | Seed admin email (first deploy) |
| `ADMIN_PASSWORD` | No | — | Seed admin password (first deploy) |
| `ADMIN_NAME` | No | `Admin` | Seed admin display name |
| `RUST_LOG` | No | `info` | Log level |

## Stack

- **Runtime:** Rust (Actix-web)
- **Database:** MySQL / MariaDB (SeaORM)
- **Auth:** JWT + Argon2 password hashing
- **Storage:** Local filesystem (dev) / Cloudflare R2 via S3 (prod)
- **WhatsApp:** Meta Business Cloud API
- **Docs:** Swagger UI via utoipa

## Features

- Multi-tenant architecture (single DB, tenant-scoped data)
- Domain-driven design (entities, factories, repositories, services)
- JWT authentication with RBAC (roles & permissions)
- Superadmin / tenant admin / agent / viewer role hierarchy
- Tenant-scoped contacts with tagging and filtering
- WhatsApp inbound webhook (text, media, status updates)
- Outbound message queue (DB-backed outbox worker)
- Message status invariant enforcement (prevents wa_message_id + failed state)
- Real-time WebSocket updates (tenant-scoped)
- Chat REST API with conversation history
- Media upload and download (local + R2)
- Per-tenant WhatsApp and R2/S3 storage configuration
- Admin API for tenant, user, and role management
- Swagger UI at `/swagger-ui/`

## Quick Start

### Prerequisites

- Rust 1.75+
- MySQL 8+ or MariaDB 10.6+
- Meta WhatsApp Business API credentials

### Setup

```bash
# Clone and enter
git clone git@github.com:igun997/crm-bro.git
cd crm-bro

# Configure environment
cp .env.example .env
# Edit .env with your database and WhatsApp credentials

# Run migrations
make migrate

# Seed superadmin
ADMIN_PASSWORD=your-secure-password make seed-admin EMAIL=admin@yourdomain.com NAME="Admin"

# Start API server
make run

# Start outbox worker (separate terminal)
make worker
```

### Development

```bash
make dev          # Auto-reload API server
make dev-worker   # Auto-reload worker
make check        # Cargo check
make test         # Run tests
make fmt          # Format code
make lint         # Clippy
```

## API Endpoints

### Auth
- `POST /api/auth/login` — JWT login

### Admin (superadmin / tenant admin)
- `POST /api/admin/tenants` — Create tenant
- `GET /api/admin/tenants/{id}/users` — List tenant users
- `POST /api/admin/tenants/{id}/users` — Create user
- `GET /api/admin/users/{id}` — Get user
- `PATCH /api/admin/users/{id}` — Update user
- `POST /api/admin/users/{id}/reset-password` — Reset password
- `POST /api/admin/users/{id}/roles` — Assign role
- `DELETE /api/admin/users/{id}/roles/{role_id}` — Remove role
- `GET /api/admin/tenants/{id}/roles` — List roles
- `POST /api/admin/tenants/{id}/roles` — Create role
- `PATCH /api/admin/roles/{id}` — Update role
- `GET /api/admin/permissions` — List permissions

### WhatsApp Settings
- `GET /api/settings/whatsapp` — Get settings
- `POST /api/settings/whatsapp` — Create settings
- `PATCH /api/settings/whatsapp/{id}` — Update settings

### Storage Settings
- `GET /api/settings/storage` — Get storage config
- `POST /api/settings/storage` — Create storage config
- `PATCH /api/settings/storage` — Update storage config

### Contacts & Tags
- `GET /api/contacts` — List contacts (filterable)
- `GET /api/contacts/{id}` — Get contact
- `PATCH /api/contacts/{id}` — Update contact
- `POST /api/contacts/{id}/tags` — Attach tag
- `DELETE /api/contacts/{id}/tags/{tag_id}` — Detach tag
- `GET /api/tags` — List tags
- `POST /api/tags` — Create tag

### Chat
- `GET /api/chat/conversations` — List conversations
- `GET /api/chat/messages/{phone}` — Message history
- `GET /api/chat/search?q=...` — Search messages
- `POST /api/chat/send/text` — Send text
- `POST /api/chat/send/template` — Send template
- `POST /api/chat/send/media` — Send media URL
- `POST /api/chat/send/upload` — Upload and send file

### WebSocket
- `ws://host/ws/updates?token=...` — Global updates
- `ws://host/ws/chat/{id}?token=...` — Conversation updates

### Webhook
- `GET /webhook/whatsapp` — Meta verification
- `POST /webhook/whatsapp` — Inbound messages

## Environment Variables

See [Docker Deployment](#docker-deployment-recommended) for full variable reference, or [`.env.example`](.env.example) for a minimal template.

## Architecture

CRM Bro uses full DDD-style layering. See [`docs/architecture/ddd-layers.md`](docs/architecture/ddd-layers.md) for layer rules and guard tests.

```
src/
├── api/
│   ├── dto/            # HTTP request/response DTOs
│   ├── middleware/     # Actix auth context/extractors
│   ├── responses.rs    # HTTP response helpers
│   └── routes/         # HTTP/WebSocket handlers
│       ├── admin.rs    # Tenant/user/role management
│       ├── auth.rs     # Login
│       ├── chat.rs     # Chat API transport mapping
│       ├── contacts.rs # Contacts & tags transport mapping
│       ├── health.rs   # Health check
│       ├── settings.rs # WhatsApp/storage settings transport mapping
│       ├── websocket.rs
│       └── whatsapp_webhook.rs
├── application/
│   ├── auth/           # RBAC enforcement use cases
│   ├── contacts/       # Contact query/filter use cases
│   ├── messaging/      # Chat send/list/search and webhook use cases
│   ├── storage/
│   └── tenants/        # Settings use cases
├── domain/
│   ├── auth/           # User, role, permission rules
│   ├── contacts/       # Contact entities/services/errors
│   ├── messaging/      # Message/Conversation/Outbox invariants
│   ├── storage/        # Storage settings rules
│   └── tenants/        # Tenant/WhatsApp/storage entities
├── infrastructure/
│   ├── config/         # AppConfig
│   ├── persistence/    # SeaORM models
│   ├── security/       # JWT/password hashing
│   ├── storage/        # Local/R2 object storage
│   ├── whatsapp/       # Meta sender/media/types
│   └── websocket/      # WebSocket actors
├── bin/
│   ├── seed_admin.rs   # Superadmin seeder CLI
│   └── worker.rs       # Outbox message worker
├── lib.rs
└── main.rs
migrations/             # SQL migrations
static/                 # Dev chat UI
```

Architecture guards live in `tests/architecture_layers.rs` and prevent legacy top-level modules from returning.

## Releases

Pushing a `v*` tag triggers:
1. Full CI (check, lint, test, build)
2. Docker image build → `ghcr.io/igun997/crm-bro`
3. GitHub Release with auto-generated changelog

```bash
# Create a release
git tag v0.1.0
git push origin v0.1.0
```

Pre-release tags (`-alpha`, `-beta`, `-rc`) are marked as pre-release and NOT tagged as `latest`.

## License

Private — All rights reserved.
