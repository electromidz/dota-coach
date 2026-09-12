# AI Dota Coach

A personal AI coach for Dota 2 players. It reads your recent matches, turns them
into hard numbers, learns the habits that repeat across games, and gives you
**one** thing to train next — not a wall of statistics.

> Status: **Phase 1 complete.** The repository structure, the Rust API, the
> database and the Next.js frontend run end to end. Match syncing, metrics,
> AI analysis and the coach itself land in Phases 2–6 (see
> [Roadmap](#roadmap)).

---

## Product overview

Most Dota tools are dashboards: they show you what happened. This one is a
coach: it looks across many games and answers four questions.

- What am I good at?
- What am I bad at?
- What keeps repeating?
- What should I work on right now?

The key idea is that matches are **not** analysed in isolation. A single bad
fight is noise. The same bad fight in four of the last ten games is a pattern,
and a pattern is something you can train.

Nothing here promises MMR gains, and every score the app shows is an estimate
derived from public match data.

---

## Architecture

```text
Dota data (OpenDota)
        │
        ▼
Provider layer         normalizes provider payloads into internal domain models
        │
        ▼
Deterministic metrics  laning / farming / fighting / survival / objectives / impact
        │
        ▼
Player history         aggregates across matches
        │
        ▼
AI analysis            LLM reasons over structured data and returns strict JSON
        │
        ▼
Coaching insights      persisted strengths, weaknesses, recurring patterns
        │
        ▼
Training focus         exactly one current focus
```

Two rules hold the design together:

1. **The LLM never does arithmetic.** All numbers are computed in Rust, are
   reproducible, and are version-stamped. The model only interprets them.
2. **Providers are replaceable.** OpenDota response shapes stop at the provider
   boundary; everything above it speaks the internal domain model, so STRATZ (or
   anything else) can be swapped in without touching handlers.

It is a **modular monolith** — one Rust binary, one database, no queues, no
service mesh.

---

## Tech stack

| Layer    | Choice                                                  |
| -------- | ------------------------------------------------------- |
| Frontend | Next.js 16 (App Router), TypeScript, Tailwind CSS 4, PWA |
| Backend  | Rust, Axum, Tokio, Serde, SQLx                          |
| Database | PostgreSQL 17                                           |
| AI       | Any OpenAI-compatible endpoint, behind a trait          |
| Dev      | Docker Compose                                          |

SQLx queries are **runtime-checked** (`sqlx::query_as`) rather than macro-checked
on purpose: the backend image must build without a live database.

---

## Project structure

```text
dota-coach/
├── docker-compose.yml
├── .env.example
├── backend/
│   ├── Cargo.toml
│   ├── Dockerfile
│   ├── migrations/            # SQLx migrations
│   └── src/
│       ├── main.rs            # startup, tracing, graceful shutdown
│       ├── config/            # env -> typed Config (secrets stay here)
│       ├── error.rs           # AppError -> consistent JSON error envelope
│       ├── state.rs           # AppState: pool + providers
│       ├── db/                # pool creation + migration runner
│       ├── api/
│       │   ├── routes.rs      # every route mounted in one place
│       │   └── handlers/
│       ├── domain/            # normalized models
│       ├── services/
│       │   ├── dota/          # DotaDataProvider trait + OpenDota impl
│       │   ├── metrics/       # deterministic scores + impact score
│       │   ├── coaching/      # profile, patterns, training focus
│       │   └── llm/           # LlmProvider trait + OpenAI-compatible impl
│       └── repositories/      # SQL access, one module per aggregate
└── frontend/
    ├── Dockerfile
    └── src/
        ├── app/               # App Router pages
        ├── components/        # dashboard / matches / coach / charts / ui
        └── lib/               # api client, shared types, utils
```

Business logic lives in the backend. React components consume the API and
render; they do not compute coaching.

---

## Local setup

### Option A — Docker (everything at once)

```bash
cp .env.example .env    # optional: defaults work out of the box
docker compose up --build
```

- Frontend → <http://localhost:3000>
- Backend → <http://localhost:8080/health>
- Postgres → `localhost:5432` (user `dota`, password `dota`, db `dota_coach`)

Migrations run automatically when the backend starts.

The first build compiles Rust in release mode and takes several minutes.
Afterwards, dependency layers are cached.

To stop and wipe the database volume:

```bash
docker compose down -v
```

### Option B — Native (recommended while developing)

Docker images are production builds with no hot reload. For day-to-day work,
run Postgres in Docker and the two apps natively.

```bash
# 1. Database only
docker compose up -d postgres

# 2. Backend  (http://localhost:8080)
cd backend
export DATABASE_URL='postgres://dota:dota@localhost:5432/dota_coach'
cargo run

# 3. Frontend (http://localhost:3000)
cd frontend
npm install
npm run dev
```

Requirements: Rust 1.85+, Node 22+, Docker (or a local PostgreSQL 17).

---

## Environment variables

Copy `.env.example` to `.env`. Never commit the real file.

| Variable                 | Used by  | Notes                                                               |
| ------------------------ | -------- | ------------------------------------------------------------------- |
| `DATABASE_URL`           | backend  | Required. Inside Compose the host is `postgres`, not `localhost`.   |
| `HOST` / `PORT`          | backend  | Defaults `0.0.0.0:8080`.                                            |
| `CORS_ORIGINS`           | backend  | Comma-separated allowlist. Defaults to `http://localhost:3000`.     |
| `RUST_LOG`               | backend  | Tracing filter.                                                     |
| `DOTA_API_BASE_URL`      | backend  | Defaults to OpenDota.                                               |
| `LLM_BASE_URL`           | backend  | Any OpenAI-compatible base URL.                                     |
| `LLM_API_KEY`            | backend  | **Server-side only.** Never prefixed with `NEXT_PUBLIC_`.           |
| `LLM_MODEL`              | backend  | Model identifier.                                                   |
| `NEXT_PUBLIC_API_URL`    | frontend | Must be reachable **from the browser**, not from inside a container. |

`NEXT_PUBLIC_*` values are inlined into the client bundle at build time, so
changing `NEXT_PUBLIC_API_URL` requires rebuilding the frontend image.

### Configuring the LLM

The backend talks to any OpenAI-compatible `/chat/completions` endpoint, so
OpenAI, a local vLLM/Ollama gateway or a hosted alternative all work:

```env
LLM_BASE_URL=https://api.openai.com/v1
LLM_API_KEY=sk-...
LLM_MODEL=gpt-4o-mini
```

If `LLM_API_KEY` is unset the backend still starts, logs a warning, and reports
`"llm_configured": false` from `/health`. Deterministic statistics keep working;
only AI analysis is unavailable. The key never leaves the backend process.

---

## Database migrations

Migrations live in `backend/migrations/` and are **embedded into the binary** at
compile time, then applied on startup — no separate migrate step in Docker.

To add one:

```bash
cd backend
# requires: cargo install sqlx-cli --no-default-features --features postgres
sqlx migrate add <name>
```

Inspect the schema:

```bash
docker exec -it dota-coach-postgres-1 psql -U dota -d dota_coach -c '\dt'
```

---

## API

Implemented today:

| Method | Path           | Description                              |
| ------ | -------------- | ---------------------------------------- |
| `GET`  | `/health`      | Readiness: version, database, LLM config |
| `GET`  | `/api/health`  | Same, under the API prefix               |
| `GET`  | `/health/live` | Liveness; touches no dependencies        |

Planned (Phases 2–5):

```text
POST   /api/players                  create or find a player by Steam/Dota ID
GET    /api/players/:id
POST   /api/players/:id/sync         fetch, store, compute metrics
GET    /api/players/:id/matches
GET    /api/players/:id/stats        aggregates + time-of-day
GET    /api/players/:id/coach        profile, patterns, current focus
GET    /api/matches/:id
POST   /api/matches/:id/analyze      AI analysis (rate-limited)
```

Errors always use one envelope, and never leak internals:

```json
{ "error": { "code": "PLAYER_NOT_FOUND", "message": "Player not found." } }
```

---

## How the AI coaching pipeline works

1. **Normalize.** The provider converts OpenDota payloads into domain models.
   Duplicate matches are prevented by a `UNIQUE (user_id, match_id)` constraint.
2. **Compute.** A metrics service derives KDA, deaths per 10 minutes, and
   0–100 laning / farming / fight / survival / objective scores, plus a
   transparent `impact_score` built from configurable weights.
3. **Aggregate.** The player's recent history is summarized: how many matches
   have been analysed, which strengths and weaknesses recur.
4. **Analyse.** The LLM receives a compact structured payload — match facts,
   computed scores, history summary — never raw API blobs, and never
   user-controlled prompt text. It must return strict JSON, which is validated
   before anything is stored.
5. **Persist.** Validated strengths and weaknesses become `coaching_insights`
   rows tagged with a category and severity.
6. **Focus.** The coach counts insight categories across recent matches and
   promotes the single most frequent weakness to the current training focus,
   with the evidence that justified it ("4 of your last 10 games").

---

## Testing

```bash
cd backend && cargo test     # config, error mapping (metrics + parsing follow)
cd frontend && npm test      # API client error handling, utils
```

---

## Roadmap

| Phase | Scope                                                         | Status |
| ----- | ------------------------------------------------------------- | ------ |
| 1     | Repo structure, Axum API, Postgres, Docker, basic frontend     | ✅ done |
| 2     | Dota provider, player lookup, match sync, schema               | next   |
| 3     | Deterministic metrics, dashboard, match page, charts           |        |
| 4     | LLM abstraction, structured match analysis, analysis UI        |        |
| 5     | Player history, recurring patterns, profile, training focus    |        |
| 6     | PWA, landing polish, error states, tests                       |        |

Deliberately out of scope: live coaching, overlays, voice, replay parsing,
native apps, social features, payments, leaderboards.
