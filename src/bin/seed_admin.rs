use clap::Parser;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Database, EntityTrait, QueryFilter,
};

use crm_bro::auth::password::hash_password;
use crm_bro::models::permission;
use crm_bro::models::role;
use crm_bro::models::role_permission;
use crm_bro::models::user;
use crm_bro::models::user_role;

/// Seed a superadmin user into the database.
#[derive(Parser, Debug)]
#[command(name = "seed_admin", about = "Create or update the superadmin user")]
struct Args {
    /// Email address for the superadmin
    #[arg(long)]
    email: String,

    /// Plaintext password (will be hashed before storage)
    #[arg(long)]
    password: String,

    /// Display name for the superadmin
    #[arg(long)]
    name: String,
}

/// All permission codes that should exist in the system.
const PERMISSIONS: &[(&str, &str)] = &[
    ("chats.read",               "Read conversations and messages"),
    ("chats.send",               "Send messages"),
    ("contacts.read",            "Read contacts"),
    ("contacts.write",           "Create and update contacts"),
    ("contacts.delete",          "Delete contacts"),
    ("tags.read",                "Read tags"),
    ("tags.write",               "Create and update tags"),
    ("settings.whatsapp.manage", "Manage WhatsApp account settings"),
    ("admin.tenants.manage",     "Create and manage tenants"),
    ("admin.users.manage",       "Create and manage users"),
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (ignore error if missing)
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in environment or .env file");

    let db = Database::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    tracing::info!("Connected to database");

    // ── 1. Seed permissions ──────────────────────────────────────────────────
    tracing::info!("Seeding permissions...");
    for (code, description) in PERMISSIONS {
        let existing = permission::Entity::find()
            .filter(permission::Column::Code.eq(*code))
            .one(&db)
            .await?;

        if existing.is_none() {
            let perm = permission::ActiveModel {
                code: Set(code.to_string()),
                description: Set(Some(description.to_string())),
                ..Default::default()
            };
            perm.insert(&db).await?;
            tracing::info!("  Created permission: {}", code);
        } else {
            tracing::info!("  Permission already exists: {}", code);
        }
    }

    // ── 2. Create/find superadmin role ───────────────────────────────────────
    tracing::info!("Ensuring superadmin role exists...");
    let superadmin_role = match role::Entity::find()
        .filter(role::Column::Name.eq("superadmin"))
        .filter(role::Column::TenantId.is_null())
        .one(&db)
        .await?
    {
        Some(r) => {
            tracing::info!("  Role 'superadmin' already exists (id={})", r.id);
            r
        }
        None => {
            let new_role = role::ActiveModel {
                tenant_id: Set(None),
                name: Set("superadmin".to_string()),
                description: Set(Some("System superadmin role with all permissions".to_string())),
                is_system: Set(true),
                ..Default::default()
            };
            let r = new_role.insert(&db).await?;
            tracing::info!("  Created role 'superadmin' (id={})", r.id);
            r
        }
    };

    // ── 3. Attach all permissions to superadmin role ─────────────────────────
    tracing::info!("Attaching permissions to superadmin role...");
    let all_perms = permission::Entity::find().all(&db).await?;
    for perm in &all_perms {
        let existing = role_permission::Entity::find()
            .filter(role_permission::Column::RoleId.eq(superadmin_role.id))
            .filter(role_permission::Column::PermissionId.eq(perm.id))
            .one(&db)
            .await?;
        if existing.is_none() {
            let rp = role_permission::ActiveModel {
                role_id: Set(superadmin_role.id),
                permission_id: Set(perm.id),
            };
            rp.insert(&db).await?;
            tracing::info!("  Attached permission '{}' to superadmin role", perm.code);
        }
    }

    // ── 4. Create/update user ────────────────────────────────────────────────
    tracing::info!("Creating/updating superadmin user '{}'...", args.email);
    let password_hash = hash_password(&args.password)
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;

    let user_model = match user::Entity::find()
        .filter(user::Column::Email.eq(&args.email))
        .one(&db)
        .await?
    {
        Some(existing) => {
            // Update existing user
            let mut active: user::ActiveModel = existing.into();
            active.name = Set(args.name.clone());
            active.password_hash = Set(password_hash);
            active.is_superadmin = Set(true);
            active.tenant_id = Set(None);
            active.is_active = Set(true);
            let updated = active.update(&db).await?;
            tracing::info!("  Updated existing user (id={})", updated.id);
            updated
        }
        None => {
            let new_user = user::ActiveModel {
                email: Set(args.email.clone()),
                name: Set(args.name.clone()),
                password_hash: Set(password_hash),
                is_superadmin: Set(true),
                tenant_id: Set(None),
                is_active: Set(true),
                ..Default::default()
            };
            let created = new_user.insert(&db).await?;
            tracing::info!("  Created new user (id={})", created.id);
            created
        }
    };

    // ── 5. Attach superadmin role to user ────────────────────────────────────
    let existing_ur = user_role::Entity::find()
        .filter(user_role::Column::UserId.eq(user_model.id))
        .filter(user_role::Column::RoleId.eq(superadmin_role.id))
        .one(&db)
        .await?;

    if existing_ur.is_none() {
        let ur = user_role::ActiveModel {
            user_id: Set(user_model.id),
            role_id: Set(superadmin_role.id),
        };
        ur.insert(&db).await?;
        tracing::info!("  Attached superadmin role to user");
    } else {
        tracing::info!("  User already has superadmin role");
    }

    println!(
        "✓ Superadmin ready — id={} email={} name={} is_superadmin=true is_active=true tenant_id=None",
        user_model.id, user_model.email, user_model.name
    );

    Ok(())
}
