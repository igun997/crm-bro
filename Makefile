.PHONY: run build check test clean fmt lint dev db-check migrate static seed-admin

run:
	RUST_LOG=debug cargo run

dev:
	RUST_LOG=debug cargo watch -i media/ -x run

build:
	cargo build --release

check:
	cargo check

test:
	cargo test

clean:
	cargo clean

fmt:
	cargo fmt

lint:
	cargo clippy -- -D warnings

db-check:
	@set -a; . ./.env; set +a; \
	eval $$(python3 -c 'import os, shlex; from urllib.parse import urlparse; u=urlparse(os.environ["DATABASE_URL"]); print(f"DB_USER={shlex.quote(u.username or "")} DB_PASSWORD={shlex.quote(u.password or "")} DB_HOST={shlex.quote(u.hostname or "localhost")} DB_NAME={shlex.quote(u.path.lstrip("/"))}")'); \
	MYSQL_PWD="$$DB_PASSWORD" mysql -u "$$DB_USER" -h "$$DB_HOST" "$$DB_NAME" -e "SELECT 'Connection successful!' as status;"

migrate:
	@echo "Running migrations..."
	@set -a; . ./.env; set +a; \
	eval $$(python3 -c 'import os, shlex; from urllib.parse import urlparse; u=urlparse(os.environ["DATABASE_URL"]); print(f"DB_USER={shlex.quote(u.username or "")} DB_PASSWORD={shlex.quote(u.password or "")} DB_HOST={shlex.quote(u.hostname or "localhost")} DB_NAME={shlex.quote(u.path.lstrip("/"))}")'); \
	for f in migrations/*.sql; do \
		echo "Applying $$f"; \
		MYSQL_PWD="$$DB_PASSWORD" mysql -u "$$DB_USER" -h "$$DB_HOST" "$$DB_NAME" < $$f; \
	done
	@echo "Done."

static:
	@echo "Serving static at http://localhost:3000"
	python3 -m http.server 3000 --directory static

seed-admin:
	cargo run --bin seed_admin -- --email "$(EMAIL)" --password-env "$${PASSWORD_ENV:-ADMIN_PASSWORD}" --name "$(NAME)"
