#!/usr/bin/env bash
set -euo pipefail

# ── Parse DATABASE_URL for mysql client ──────────────────────────────────────
parse_db_url() {
  # mysql://user:pass@host:port/dbname
  local url="${DATABASE_URL:-}"
  if [[ -z "$url" ]]; then
    echo "ERROR: DATABASE_URL is required" >&2
    exit 1
  fi

  DB_USER=$(echo "$url" | sed -E 's|^mysql://([^:]+):.*|\1|')
  DB_PASS=$(echo "$url" | sed -E 's|^mysql://[^:]+:([^@]+)@.*|\1|')
  DB_HOST=$(echo "$url" | sed -E 's|^mysql://[^@]+@([^:/]+).*|\1|')
  DB_PORT=$(echo "$url" | sed -E 's|^mysql://[^@]+@[^:]+:([0-9]+)/.*|\1|' || echo "3306")
  DB_NAME=$(echo "$url" | sed -E 's|^mysql://[^/]+/([^?]+).*|\1|')

  # Default port if not in URL
  if [[ "$DB_PORT" == "$url" ]] || [[ -z "$DB_PORT" ]]; then
    DB_PORT=3306
  fi
}

# ── Wait for database ───────────────────────────────────────────────────────
wait_for_db() {
  echo "⏳ Waiting for database at $DB_HOST:$DB_PORT..."
  local retries=30
  while ! MYSQL_PWD="$DB_PASS" mysqladmin ping -h "$DB_HOST" -P "$DB_PORT" -u "$DB_USER" --silent 2>/dev/null; do
    retries=$((retries - 1))
    if [[ $retries -le 0 ]]; then
      echo "ERROR: Database not reachable after 30 attempts" >&2
      exit 1
    fi
    sleep 2
  done
  echo "✅ Database is ready"
}

# ── Run migrations ──────────────────────────────────────────────────────────
run_migrations() {
  echo "🔄 Running migrations..."
  for f in /app/migrations/*.sql; do
    echo "  Applying $(basename "$f")"
    MYSQL_PWD="$DB_PASS" mysql -h "$DB_HOST" -P "$DB_PORT" -u "$DB_USER" "$DB_NAME" < "$f"
  done
  echo "✅ Migrations complete"
}

# ── Seed admin (first deploy) ───────────────────────────────────────────────
maybe_seed_admin() {
  if [[ -n "${ADMIN_EMAIL:-}" ]] && [[ -n "${ADMIN_PASSWORD:-}" ]]; then
    echo "👤 Seeding superadmin user..."
    /usr/local/bin/seed_admin \
      --email "$ADMIN_EMAIL" \
      --password-env ADMIN_PASSWORD \
      --name "${ADMIN_NAME:-Admin}"
    echo "✅ Superadmin seeded"
  fi
}

# ── Main ────────────────────────────────────────────────────────────────────
parse_db_url

case "${1:-api}" in
  api)
    wait_for_db
    run_migrations
    maybe_seed_admin
    echo "🚀 Starting CRM-Bro API server..."
    exec crm-bro
    ;;
  worker)
    wait_for_db
    echo "🔄 Starting outbox worker..."
    exec worker
    ;;
  seed)
    wait_for_db
    run_migrations
    maybe_seed_admin
    echo "✅ Seed complete"
    ;;
  migrate)
    wait_for_db
    run_migrations
    ;;
  *)
    # Pass through to exec arbitrary commands
    exec "$@"
    ;;
esac
