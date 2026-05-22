.PHONY: run build check test clean fmt lint dev

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
	mysql -u REDACTED_DB_USER -p'REDACTED_DB_PASSWORD' -h localhost crmbro -e "SELECT 'Connection successful!' as status;"

migrate:
	@echo "Running migrations..."
	@for f in migrations/*.sql; do \
		echo "Applying $$f"; \
		mysql -u REDACTED_DB_USER -p'REDACTED_DB_PASSWORD' -h localhost crmbro < $$f; \
	done
	@echo "Done."
