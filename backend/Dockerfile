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
    && cargo build --release \
    && rm -rf src

COPY migrations ./migrations
COPY src ./src
# Touch so cargo rebuilds the binary rather than reusing the stub.
RUN touch src/main.rs && cargo build --release

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
