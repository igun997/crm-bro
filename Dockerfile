# ── Stage 1: Build ────────────────────────────────────────────────────────────
FROM rust:1-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# Create dummy src to cache deps
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    mkdir -p src/bin && \
    echo "fn main() {}" > src/bin/seed_admin.rs && \
    echo "fn main() {}" > src/bin/worker.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

COPY src ./src
COPY static ./static
COPY migrations ./migrations
# Touch to invalidate caches for actual source
RUN touch src/main.rs src/lib.rs src/bin/seed_admin.rs src/bin/worker.rs
RUN cargo build --release --bin crm-bro --bin seed_admin --bin worker

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      ca-certificates \
      default-mysql-client \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binaries
COPY --from=builder /app/target/release/crm-bro /usr/local/bin/crm-bro
COPY --from=builder /app/target/release/seed_admin /usr/local/bin/seed_admin
COPY --from=builder /app/target/release/worker /usr/local/bin/worker

# Copy migrations and static assets
COPY migrations ./migrations
COPY static ./static

# Copy entrypoint
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Default env
ENV RUST_LOG=info
ENV STORAGE_BACKEND=r2
ENV APP_BASE_URL=http://localhost:8080

EXPOSE 8080

ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["api"]
