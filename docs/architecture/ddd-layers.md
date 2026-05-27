# DDD Layers

CRM Bro uses four explicit layers under `src/`:

```text
src/
├── api/              # HTTP/WebSocket entrypoints, DTOs, middleware, response mapping
├── application/      # Use cases and orchestration across domain + adapters
├── domain/           # Business entities, value objects, services, invariants
└── infrastructure/   # Config, persistence models, security, storage, WhatsApp, WebSocket actors
```

## Layer responsibilities

### `api`

Owns transport concerns:

- Actix routes and middleware
- request parsing and response DTOs
- auth extractor and HTTP permission checks
- Swagger/OpenAPI annotations
- webhook endpoint shape and Meta-compatible HTTP behavior

API calls application use cases and maps outputs to HTTP responses.

### `application`

Owns use-case orchestration:

- contact list/filter/tag queries
- settings create/update/list flows
- chat list/search/send flows
- webhook inbound/status processing
- RBAC permission enforcement helpers

Application may coordinate persistence models and infrastructure adapters while keeping routes thin.

### `domain`

Owns business rules:

- auth permissions and user entity behavior
- contacts entities/services/errors
- messaging status invariants, conversation/message/outbox state
- tenant/storage/WhatsApp setting entities

Domain must not import transport or external client frameworks such as `actix_web`, `reqwest`, `object_store`, or `crate::api`.

Current note: limited persistence coupling remains in known mapper/repository files and is guarded by `tests/architecture_layers.rs` until mappers are fully moved out.

### `infrastructure`

Owns external systems and technical adapters:

- `config` — `AppConfig`
- `persistence/models` — SeaORM entities
- `security` — JWT/password hashing
- `storage` — local/R2 object storage
- `whatsapp` — Meta sender/media/types
- `websocket` — Actix hub/session actors

## Removed legacy top-level modules

Guard tests prevent these old paths from returning:

- `src/auth`
- `src/config`
- `src/middleware`
- `src/models`
- `src/rbac`
- `src/storage`
- `src/whatsapp`
- `src/ws`
- `src/common`
- `src/response.rs`

## Architecture guard

Run:

```bash
cargo test --test architecture_layers
```

Guard file:

- `tests/architecture_layers.rs`

It enforces legacy path removal and domain layer boundaries.
