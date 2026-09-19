# ---- builder -----------------------------------------------------------
FROM rust:1-slim-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Build dependencies against a stub main so that editing src/ does not
# invalidate the (slow) dependency layer.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release --locked --bin dota-coach-backend \
    && rm -rf src

# `sqlx::migrate!` embeds this directory at compile time, so it has to be
# present before the build, not just at runtime.
COPY migrations ./migrations
COPY src ./src
# Touch so cargo rebuilds the binary rather than reusing the stub. Only the
# serving binary is built: src/bin/backfill_subscriptions.rs is a one-off
# operator tool that the runtime stage does not copy, and building it here
# would cost a second link for something the image never contains.
RUN touch src/main.rs && cargo build --release --locked --bin dota-coach-backend

# ---- runtime -----------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 app

WORKDIR /app
COPY --from=builder /app/target/release/dota-coach-backend /usr/local/bin/dota-coach-backend

USER app
EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --start-period=20s --retries=5 \
    CMD curl -fsS http://127.0.0.1:8080/health/live || exit 1

CMD ["dota-coach-backend"]
