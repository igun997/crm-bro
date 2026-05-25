# MVP4: DDD Refactor Design

**Date:** 2026-05-25  
**Status:** Approved  
**Driver:** Full DDD transformation (scalability, testability, team growth, code organization)

## Approach

**Pragmatic DDD** — Domain services + repositories pattern without strict hexagonal ceremony. Balance between purity and practicality.

**Cross-cutting:** Hybrid — Shared infrastructure (auth, DB, config) but domain-specific errors/DTOs.

**Migration:** Incremental by domain — One domain at a time, smaller PRs, lower risk.

## Domains

1. **Auth** — Users, JWT, sessions, roles, permissions
2. **Tenants** — Multi-tenancy, tenant config, WhatsApp settings, storage settings
3. **Contacts** — Contact management, tags
4. **Messaging** — Conversations, messages, WhatsApp send/receive, outbox
5. **Storage** — R2/S3/local media handling

## Target Structure

```
src/
├── common/                    # Shared infrastructure
│   ├── auth/                  # JWT, password, context, extractor
│   ├── config/                # AppConfig
│   ├── db/                    # Connection pool, transaction helpers
│   ├── error/                 # Base error types, response helpers
│   └── middleware/            # CORS, logging, etc.
│
├── domain/
│   ├── auth/                  # Domain 1
│   │   ├── entities/          # User (with factory)
│   │   ├── services/          # AuthService (login, register)
│   │   ├── repositories/      # UserRepository trait + impl
│   │   ├── errors.rs          # Domain-specific errors
│   │   └── mod.rs
│   │
│   ├── tenants/               # Domain 2
│   │   ├── entities/          # Tenant, TenantWhatsAppAccount, TenantStorageConfig
│   │   ├── services/          # TenantService, SettingsService
│   │   ├── repositories/
│   │   ├── errors.rs
│   │   └── mod.rs
│   │
│   ├── contacts/              # Domain 3
│   │   ├── entities/          # Contact, Tag, ContactTag
│   │   ├── services/          # ContactService, TagService
│   │   ├── repositories/
│   │   ├── errors.rs
│   │   └── mod.rs
│   │
│   ├── messaging/             # Domain 4
│   │   ├── entities/          # Conversation, Message, OutboxMessage
│   │   ├── services/          # ChatService, OutboxService, WebhookService
│   │   ├── repositories/
│   │   ├── errors.rs
│   │   └── mod.rs
│   │
│   └── storage/               # Domain 5
│       ├── services/          # StorageService (R2/S3/Local)
│       ├── errors.rs
│       └── mod.rs
│
├── api/                       # HTTP layer (thin)
│   ├── routes/                # Route handlers call domain services
│   │   ├── auth.rs
│   │   ├── admin.rs
│   │   ├── contacts.rs
│   │   ├── chat.rs
│   │   ├── settings.rs
│   │   └── health.rs
│   ├── dto/                   # Request/Response DTOs with Swagger
│   └── mod.rs
│
├── bin/
│   ├── api.rs                 # main.rs → api.rs
│   ├── worker.rs
│   └── seed_admin.rs
│
└── lib.rs
```

## Domain Module Pattern

### Entity with Factory

```rust
// domain/contacts/entities/contact.rs
pub struct Contact {
    id: i32,
    tenant_id: i32,
    name: String,
    phone: String,
    email: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl Contact {
    /// Factory with validation
    pub fn new(tenant_id: i32, name: String, phone: String) -> Result<Self, ContactError> {
        if name.trim().is_empty() {
            return Err(ContactError::InvalidName("Name cannot be empty".into()));
        }
        if !Self::validate_phone(&phone) {
            return Err(ContactError::InvalidPhone(phone));
        }
        Ok(Self {
            id: 0,
            tenant_id,
            name: name.trim().to_string(),
            phone: Self::normalize_phone(&phone),
            email: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    /// Reconstitute from persistence (no validation)
    pub(crate) fn from_record(record: ContactRecord) -> Self {
        Self {
            id: record.id,
            tenant_id: record.tenant_id,
            name: record.name,
            phone: record.phone,
            email: record.email,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }

    /// Domain behavior
    pub fn update_name(&mut self, name: String) -> Result<(), ContactError> {
        if name.trim().is_empty() {
            return Err(ContactError::InvalidName("Name cannot be empty".into()));
        }
        self.name = name.trim().to_string();
        self.updated_at = Utc::now();
        Ok(())
    }

    fn validate_phone(phone: &str) -> bool {
        phone.len() >= 10 && phone.chars().all(|c| c.is_ascii_digit() || c == '+')
    }

    fn normalize_phone(phone: &str) -> String {
        phone.chars().filter(|c| c.is_ascii_digit()).collect()
    }
}
```

### Repository Trait + Implementation

```rust
// domain/contacts/repositories/contact_repository.rs
use async_trait::async_trait;

#[async_trait]
pub trait ContactRepository: Send + Sync {
    async fn find_by_id(&self, id: i32, tenant_id: i32) -> Result<Option<Contact>, ContactError>;
    async fn find_by_phone(&self, phone: &str, tenant_id: i32) -> Result<Option<Contact>, ContactError>;
    async fn list(&self, tenant_id: i32, pagination: Pagination) -> Result<Vec<Contact>, ContactError>;
    async fn save(&self, contact: &Contact) -> Result<Contact, ContactError>;
    async fn delete(&self, id: i32, tenant_id: i32) -> Result<(), ContactError>;
}

// SeaORM implementation
pub struct SeaOrmContactRepository {
    db: DatabaseConnection,
}

impl SeaOrmContactRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ContactRepository for SeaOrmContactRepository {
    async fn find_by_id(&self, id: i32, tenant_id: i32) -> Result<Option<Contact>, ContactError> {
        let record = contact::Entity::find()
            .filter(contact::Column::Id.eq(id))
            .filter(contact::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(|e| ContactError::Database(e.to_string()))?;
        
        Ok(record.map(Contact::from_record))
    }

    async fn save(&self, contact: &Contact) -> Result<Contact, ContactError> {
        // Insert or update logic
        // ...
    }
    
    // ... other methods
}
```

### Domain Service

```rust
// domain/contacts/services/contact_service.rs
pub struct ContactService<R: ContactRepository> {
    repo: R,
}

impl<R: ContactRepository> ContactService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub async fn create(
        &self,
        tenant_id: i32,
        name: String,
        phone: String,
    ) -> Result<Contact, ContactError> {
        // Check for duplicate phone
        if let Some(_) = self.repo.find_by_phone(&phone, tenant_id).await? {
            return Err(ContactError::DuplicatePhone(phone));
        }
        
        // Create via factory (validates)
        let contact = Contact::new(tenant_id, name, phone)?;
        
        // Persist
        self.repo.save(&contact).await
    }

    pub async fn get(&self, id: i32, tenant_id: i32) -> Result<Contact, ContactError> {
        self.repo
            .find_by_id(id, tenant_id)
            .await?
            .ok_or(ContactError::NotFound(id))
    }
}
```

### Domain Errors

```rust
// domain/contacts/errors.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContactError {
    #[error("Contact not found: {0}")]
    NotFound(i32),
    
    #[error("Invalid name: {0}")]
    InvalidName(String),
    
    #[error("Invalid phone: {0}")]
    InvalidPhone(String),
    
    #[error("Duplicate phone: {0}")]
    DuplicatePhone(String),
    
    #[error("Database error: {0}")]
    Database(String),
}

impl ContactError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::InvalidName(_) | Self::InvalidPhone(_) => 400,
            Self::DuplicatePhone(_) => 409,
            Self::Database(_) => 500,
        }
    }
}

// For Actix-web
impl From<ContactError> for HttpResponse {
    fn from(err: ContactError) -> Self {
        let status = StatusCode::from_u16(err.status_code()).unwrap();
        HttpResponse::build(status).json(serde_json::json!({
            "success": false,
            "error": err.to_string()
        }))
    }
}
```

### Thin Route Handler

```rust
// api/routes/contacts.rs
pub async fn create_contact(
    current: CurrentUser,
    db: web::Data<DatabaseConnection>,
    body: web::Json<CreateContactRequest>,
) -> HttpResponse {
    let ctx = &current.0;
    if let Err(response) = require_permission(ctx, permissions::CONTACTS_CREATE) {
        return response;
    }
    let Some(tenant_id) = ctx.tenant_id else {
        return forbidden("Tenant context required");
    };

    let repo = SeaOrmContactRepository::new(db.get_ref().clone());
    let service = ContactService::new(repo);

    match service.create(tenant_id, body.name.clone(), body.phone.clone()).await {
        Ok(contact) => HttpResponse::Created().json(ContactResponse::from(contact)),
        Err(e) => e.into(),
    }
}
```

## Key Decisions

| Aspect | Decision |
|--------|----------|
| Repository trait | Yes, for testability (mock in unit tests) |
| SeaORM models | Keep as-is in `models/`, map to/from domain entities |
| Validation | In entity factories, not routes |
| Errors | Per-domain enums, implement `Into<HttpResponse>` |
| DTOs | In `api/dto/`, separate from domain |
| Transactions | Service layer owns transaction boundaries |
| `from_record()` | `pub(crate)` — only repo can reconstitute entities |

## Migration Order

1. **Storage** (smallest, no entity deps) — Prove pattern works
2. **Auth** (users, roles, permissions, JWT)
3. **Tenants** (tenant, whatsapp config, storage config)
4. **Contacts** (contact, tag, contact_tag)
5. **Messaging** (conversation, message, outbox) — Largest, last

Each domain = 1 PR. Tests must pass before moving to next.

## Testing Strategy

```rust
// Unit test with mock repository
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    mock! {
        ContactRepo {}
        
        #[async_trait]
        impl ContactRepository for ContactRepo {
            async fn find_by_id(&self, id: i32, tenant_id: i32) -> Result<Option<Contact>, ContactError>;
            async fn find_by_phone(&self, phone: &str, tenant_id: i32) -> Result<Option<Contact>, ContactError>;
            async fn save(&self, contact: &Contact) -> Result<Contact, ContactError>;
            // ...
        }
    }

    #[tokio::test]
    async fn test_create_contact_validates_phone() {
        let mut mock_repo = MockContactRepo::new();
        mock_repo.expect_find_by_phone().returning(|_, _| Ok(None));
        
        let service = ContactService::new(mock_repo);
        let result = service.create(1, "John".into(), "invalid".into()).await;
        
        assert!(matches!(result, Err(ContactError::InvalidPhone(_))));
    }
}
```

## Dependencies to Add

```toml
[dependencies]
async-trait = "0.1"
thiserror = "1.0"

[dev-dependencies]
mockall = "0.11"
```
