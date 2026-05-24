# CRM-Bro

[![CI](https://github.com/igun997/crm-bro/actions/workflows/ci.yml/badge.svg)](https://github.com/igun997/crm-bro/actions/workflows/ci.yml)

Multi-tenant WhatsApp CRM built with Rust.

## Stack

- **Runtime:** Rust (Actix-web)
- **Database:** MySQL / MariaDB (SeaORM)
- **Auth:** JWT + Argon2 password hashing
- **Storage:** Local filesystem (dev) / Cloudflare R2 via S3 (prod)
- **WhatsApp:** Meta Business Cloud API
- **Docs:** Swagger UI via utoipa

## Features

- Multi-tenant architecture (single DB, tenant-scoped data)
- JWT authentication with RBAC (roles & permissions)
- Superadmin / tenant admin / agent / viewer role hierarchy
- Tenant-scoped contacts with tagging and filtering
- WhatsApp inbound webhook (text, media, status updates)
- Outbound message queue (DB-backed outbox worker)
- Real-time WebSocket updates (tenant-scoped)
- Chat REST API with conversation history
- Media upload and download (local + R2)
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

See [`.env.example`](.env.example) for all configuration options.

## Architecture

```
src/
├── auth/          # JWT, password hashing, extractor
├── bin/
│   ├── seed_admin.rs   # Superadmin seeder CLI
│   └── worker.rs       # Outbox message worker
├── config/        # App configuration
├── middleware/     # Token validation
├── models/        # SeaORM entities
├── rbac/          # Role/permission constants & helpers
├── routes/        # HTTP handlers
│   ├── admin.rs   # Tenant/user/role management
│   ├── auth.rs    # Login
│   ├── chat.rs    # Chat API
│   ├── contacts.rs # Contacts & tags
│   ├── health.rs  # Health check
│   └── settings.rs # WhatsApp settings
├── storage/       # Local/R2 storage abstraction
├── whatsapp/      # Meta API client, webhook, media
├── ws/            # WebSocket hub & sessions
├── lib.rs
├── main.rs
└── response.rs
migrations/        # SQL migrations
static/            # Dev chat UI
```

## License

Private — All rights reserved.
