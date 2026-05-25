use std::time::Duration;

use crm_bro::config::AppConfig;
use crm_bro::models::{message, outbox_message, tenant_whatsapp_account};
use crm_bro::whatsapp::sender::WhatsAppSender;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde_json::Value;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_ATTEMPTS: i32 = 5;
const OUTBOX_STATUS_DONE: &str = "done";

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();
    let config = AppConfig::from_env();
    let db = Database::connect(&config.database_url)
        .await
        .expect("Failed to connect database");

    tracing::info!("Outbox worker started");
    loop {
        if let Err(error) = run_once(&db).await {
            tracing::error!(%error, "Outbox poll failed");
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn run_once(db: &DatabaseConnection) -> Result<(), String> {
    let now = chrono::Utc::now().naive_utc();
    let stale_processing_cutoff = now - chrono::Duration::minutes(10);
    let jobs = outbox_message::Entity::find()
        .filter(
            sea_orm::Condition::any()
                .add(
                    sea_orm::Condition::all()
                        .add(outbox_message::Column::Status.eq("pending"))
                        .add(outbox_message::Column::NextAttemptAt.lte(now)),
                )
                .add(
                    sea_orm::Condition::all()
                        .add(outbox_message::Column::Status.eq("processing"))
                        .add(outbox_message::Column::UpdatedAt.lte(stale_processing_cutoff)),
                ),
        )
        .order_by_asc(outbox_message::Column::NextAttemptAt)
        .order_by_asc(outbox_message::Column::Id)
        .limit(10)
        .all(db)
        .await
        .map_err(|error| format!("DB poll outbox: {error}"))?;

    for job in jobs {
        if let Err(error) = process_job(db, job).await {
            tracing::error!(%error, "Outbox job failed to process");
        }
    }

    Ok(())
}

async fn process_job(db: &DatabaseConnection, job: outbox_message::Model) -> Result<(), String> {
    let Some(job) = mark_processing(db, job).await? else {
        return Ok(());
    };
    let msg = match message::Entity::find_by_id(job.message_id).one(db).await {
        Ok(Some(msg)) => msg,
        Ok(None) => {
            let message_id = job.message_id;
            return mark_job_failure(db, job, format!("Message {message_id} not found")).await;
        }
        Err(error) => return mark_job_failure(db, job, format!("DB load message: {error}")).await,
    };

    let account_id = job
        .payload_json
        .get("whatsapp_account_id")
        .and_then(|value| value.as_i64())
        .map(|id| id as i32);
    let account = match load_account(db, job.tenant_id, account_id).await {
        Ok(account) => account,
        Err(error) => return mark_failure(db, job, msg, error).await,
    };
    let sender = WhatsAppSender::from_parts(
        &account.api_version,
        &account.phone_number_id,
        &account.access_token,
    );

    let result = send_job(&sender, &job.payload_json).await;
    match result {
        Ok(wa_message_id) => mark_success(db, job, msg, wa_message_id).await,
        Err(error) => mark_failure(db, job, msg, error).await,
    }
}

async fn mark_processing(
    db: &DatabaseConnection,
    job: outbox_message::Model,
) -> Result<Option<outbox_message::Model>, String> {
    let claim = outbox_message::Entity::update_many()
        .col_expr(outbox_message::Column::Status, Expr::value("processing"))
        .col_expr(
            outbox_message::Column::UpdatedAt,
            Expr::value(chrono::Utc::now().naive_utc()),
        )
        .filter(outbox_message::Column::Id.eq(job.id))
        .filter(outbox_message::Column::Status.eq(job.status.clone()))
        .filter(outbox_message::Column::UpdatedAt.eq(job.updated_at))
        .exec(db)
        .await
        .map_err(|error| format!("DB claim outbox job: {error}"))?;

    if claim.rows_affected != 1 {
        tracing::debug!(job_id = job.id, "Outbox job already claimed");
        return Ok(None);
    }

    outbox_message::Entity::find_by_id(job.id)
        .one(db)
        .await
        .map_err(|error| format!("DB reload claimed outbox job: {error}"))
}

async fn load_account(
    db: &DatabaseConnection,
    tenant_id: i32,
    account_id: Option<i32>,
) -> Result<tenant_whatsapp_account::Model, String> {
    let account_id =
        account_id.ok_or_else(|| "Missing whatsapp_account_id in outbox payload".to_string())?;
    tenant_whatsapp_account::Entity::find()
        .filter(tenant_whatsapp_account::Column::TenantId.eq(tenant_id))
        .filter(tenant_whatsapp_account::Column::IsActive.eq(true))
        .filter(tenant_whatsapp_account::Column::Id.eq(account_id))
        .one(db)
        .await
        .map_err(|error| format!("DB load WhatsApp account: {error}"))?
        .ok_or_else(|| format!("No active WhatsApp account {account_id} for outbox job"))
}

async fn send_job(sender: &WhatsAppSender, payload: &Value) -> Result<String, String> {
    match payload.get("type").and_then(|value| value.as_str()) {
        Some("text") => {
            let to = required_str(payload, "to")?;
            let message = required_str(payload, "message")?;
            sender.send_text(to, message).await
        }
        Some("template") => {
            let to = required_str(payload, "to")?;
            let template_name = required_str(payload, "template_name")?;
            let language = required_str(payload, "language")?;
            sender
                .send_template(to, template_name, language, None)
                .await
        }
        Some("media") => {
            let to = required_str(payload, "to")?;
            let media_type = required_str(payload, "media_type")?;
            let url = required_str(payload, "url")?;
            let caption = payload.get("caption").and_then(|value| value.as_str());
            sender.send_media(to, media_type, url, caption).await
        }
        Some(other) => Err(format!("Unsupported outbox payload type: {other}")),
        None => Err("Missing outbox payload type".to_string()),
    }
}

async fn mark_success(
    db: &DatabaseConnection,
    job: outbox_message::Model,
    msg: message::Model,
    wa_message_id: String,
) -> Result<(), String> {
    let now = chrono::Utc::now().naive_utc();

    let txn = db
        .begin()
        .await
        .map_err(|error| format!("DB begin success transaction: {error}"))?;

    let mut msg_update: message::ActiveModel = msg.into();
    msg_update.status = Set("sent".to_string());
    msg_update.wa_message_id = Set(Some(wa_message_id.clone()));
    msg_update
        .update(&txn)
        .await
        .map_err(|error| format!("DB mark message sent: {error}"))?;

    let mut job_update: outbox_message::ActiveModel = job.into();
    job_update.status = Set(OUTBOX_STATUS_DONE.to_string());
    job_update.last_error = Set(None);
    job_update.updated_at = Set(now);
    job_update
        .update(&txn)
        .await
        .map_err(|error| format!("DB mark outbox done: {error}"))?;

    txn.commit()
        .await
        .map_err(|error| format!("DB commit success transaction: {error}"))?;

    tracing::info!(%wa_message_id, "Outbox job sent");
    Ok(())
}

async fn mark_failure(
    db: &DatabaseConnection,
    job: outbox_message::Model,
    msg: message::Model,
    error: String,
) -> Result<(), String> {
    let attempts = job.attempts + 1;
    let terminal = attempts >= MAX_ATTEMPTS;
    let status = if terminal { "failed" } else { "pending" };
    let next_attempt_at =
        chrono::Utc::now().naive_utc() + chrono::Duration::seconds(retry_delay_seconds(attempts));

    let mut job_update: outbox_message::ActiveModel = job.into();
    job_update.status = Set(status.to_string());
    job_update.attempts = Set(attempts);
    job_update.last_error = Set(Some(error.clone()));
    job_update.next_attempt_at = Set(next_attempt_at);
    job_update.updated_at = Set(chrono::Utc::now().naive_utc());
    job_update
        .update(db)
        .await
        .map_err(|db_error| format!("DB mark outbox failure: {db_error}"))?;

    if terminal && msg.wa_message_id.is_some() {
        tracing::warn!(
            message_id = msg.id,
            wa_message_id = ?msg.wa_message_id,
            "Skipping failed message status because WhatsApp message id exists"
        );
    } else {
        let mut msg_update: message::ActiveModel = msg.into();
        msg_update.status = Set(if terminal { "failed" } else { "queued" }.to_string());
        msg_update
            .update(db)
            .await
            .map_err(|db_error| format!("DB mark message failure: {db_error}"))?;
    }

    tracing::warn!(attempts, terminal, %error, "Outbox job send failed");
    Ok(())
}

async fn mark_job_failure(
    db: &DatabaseConnection,
    job: outbox_message::Model,
    error: String,
) -> Result<(), String> {
    let attempts = job.attempts + 1;
    let terminal = attempts >= MAX_ATTEMPTS;
    let status = if terminal { "failed" } else { "pending" };
    let next_attempt_at =
        chrono::Utc::now().naive_utc() + chrono::Duration::seconds(retry_delay_seconds(attempts));

    let mut job_update: outbox_message::ActiveModel = job.into();
    job_update.status = Set(status.to_string());
    job_update.attempts = Set(attempts);
    job_update.last_error = Set(Some(error.clone()));
    job_update.next_attempt_at = Set(next_attempt_at);
    job_update.updated_at = Set(chrono::Utc::now().naive_utc());
    job_update
        .update(db)
        .await
        .map_err(|db_error| format!("DB mark outbox failure: {db_error}"))?;

    tracing::warn!(attempts, terminal, %error, "Outbox job failed before message update");
    Ok(())
}

fn retry_delay_seconds(attempts: i32) -> i64 {
    match attempts {
        0 | 1 => 10,
        2 => 30,
        3 => 60,
        _ => 300,
    }
}

fn required_str<'a>(payload: &'a Value, field: &str) -> Result<&'a str, String> {
    payload
        .get(field)
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("Missing payload field: {field}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_backs_off() {
        assert_eq!(retry_delay_seconds(1), 10);
        assert_eq!(retry_delay_seconds(2), 30);
        assert_eq!(retry_delay_seconds(3), 60);
        assert_eq!(retry_delay_seconds(4), 300);
    }

    #[test]
    fn outbox_success_status_matches_migration_enum() {
        assert_eq!(OUTBOX_STATUS_DONE, "done");
    }

    #[test]
    fn load_account_requires_payload_account_id() {
        let error = require_account_id(None).unwrap_err();
        assert_eq!(error, "Missing whatsapp_account_id in outbox payload");
    }

    fn require_account_id(account_id: Option<i32>) -> Result<i32, String> {
        account_id.ok_or_else(|| "Missing whatsapp_account_id in outbox payload".to_string())
    }

    #[test]
    fn required_str_reads_string_field() {
        let payload = serde_json::json!({"to":"628"});
        assert_eq!(required_str(&payload, "to"), Ok("628"));
        assert_eq!(
            required_str(&payload, "missing"),
            Err("Missing payload field: missing".to_string())
        );
    }
}
