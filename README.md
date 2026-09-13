# AI Dota Coach

A personal AI coach for Dota 2 players. It reads your recent matches, turns them
into hard numbers, learns the habits that repeat across games, and gives you
**one** thing to train next — not a wall of statistics.

> Status: **Phase 3 complete.** Sign in with Steam, the backend links your Dota
> account from the proven Steam identity, syncs your recent matches into
> Postgres, and the frontend shows your profile and paginated history.
> Deterministic metrics, AI analysis and the coach itself land in Phases 4–6
> (see [Roadmap](#roadmap)).

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
Steam OpenID          proves who the user is; nothing else may set identity
        │
        ▼
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
3. **Identity comes from the session, always.** The Steam id is written only
   from a verified OpenID assertion, and every query is scoped to the caller's
   own player row. No endpoint accepts a user, player or Steam id as input.

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
│       ├── lib.rs             # module tree, so tests can build the real router
│       ├── config/            # env -> typed Config (secrets stay here)
│       ├── error.rs           # AppError -> consistent JSON error envelope
│       ├── state.rs           # AppState: pool + providers
│       ├── db/                # pool creation + migration runner
│       ├── api/
│       │   ├── routes.rs      # every route mounted in one place
│       │   ├── extract.rs     # CurrentUser + rejections via the error envelope
│       │   └── handlers/
│       ├── domain/            # user, session, player, match, role
│       ├── services/
│       │   ├── auth/          # Steam OpenID + server-side sessions
│       │   ├── dota/          # DotaDataProvider trait + OpenDota impl
│       │   ├── sync/          # fetch -> dedupe -> enrich -> store
│       │   ├── metrics/       # deterministic scores + impact score
│       │   ├── coaching/      # profile, patterns, training focus
│       │   └── llm/           # LlmProvider trait + OpenAI-compatible impl
│       └── repositories/      # SQL access, one module per aggregate
└── frontend/
    ├── Dockerfile
    └── src/
        ├── app/               # App Router pages (/, /matches/[id])
        ├── components/        # dashboard / matches / ui
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

### Signing in

Open <http://localhost:3000> and click **Sign in with Steam**. Steam accepts a
`localhost` `return_to`, so the real OpenID flow works in development with no
Steam API key and no app registration — `PUBLIC_BASE_URL` just has to match the
address the browser actually uses.

Then hit **Sync Matches** to pull your recent games. Your Dota account is
resolved from the Steam identity automatically; there is nothing to type in.

---

## Environment variables

Copy `.env.example` to `.env`. Never commit the real file.

| Variable                 | Used by  | Notes                                                               |
| ------------------------ | -------- | ------------------------------------------------------------------- |
| `DATABASE_URL`           | backend  | Required. Inside Compose the host is `postgres`, not `localhost`.   |
| `HOST` / `PORT`          | backend  | Defaults `0.0.0.0:8080`.                                            |
| `CORS_ORIGINS`           | backend  | Comma-separated allowlist. Credentials are allowed, so no wildcard. |
| `PUBLIC_BASE_URL`        | backend  | Public origin of the API. Steam signs `return_to`; must match the browser's view. |
| `FRONTEND_BASE_URL`      | backend  | Where login redirects land, and where failures return `?error=`.    |
| `STEAM_OPENID_URL`       | backend  | Valve's endpoint. Override only in tests.                           |
| `SESSION_TTL_HOURS`      | backend  | Session lifetime. Default 720 (30 days).                            |
| `COOKIE_SECURE`          | backend  | **Set `true` on HTTPS.** `false` only for plain-HTTP localhost.     |
| `RUST_LOG`               | backend  | Tracing filter.                                                     |
| `DOTA_API_BASE_URL`      | backend  | Defaults to OpenDota.                                               |
| `DOTA_API_KEY`           | backend  | Optional. Raises OpenDota's rate limit.                             |
| `SYNC_MATCH_LIMIT`       | backend  | Matches pulled per sync, 1-100. Default 20.                         |
| `SYNC_COOLDOWN_SECONDS`  | backend  | Per-player sync throttle. Default 30; `0` disables.                 |
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

Everything under `/api` except `/api/health` requires a session. The current
user is resolved from the session cookie on **every** request; no endpoint
accepts a user id, player id or Steam id from the caller.

### Authentication

| Method | Path                    | Description                                       |
| ------ | ----------------------- | ------------------------------------------------- |
| `GET`  | `/auth/steam/login`     | Redirects to Steam. Sets the login nonce cookie.  |
| `GET`  | `/auth/steam/callback`  | Steam returns here; sets the session cookie       |
| `GET`  | `/api/auth/session`     | The signed-in account, or `401`                   |
| `POST` | `/api/auth/logout`      | Destroys the session and clears the cookie        |

The two login endpoints are reached by **top-level browser navigation** and
answer with redirects, not JSON — OpenID cannot be completed from `fetch`.

### Player and matches

| Method | Path                      | Description                                          |
| ------ | ------------------------- | ---------------------------------------------------- |
| `GET`  | `/api/players/me`         | Steam profile + linked Dota identity + match count   |
| `POST` | `/api/players/me/sync`    | Fetch, deduplicate and store recent matches          |
| `GET`  | `/api/matches`            | Own history, newest first. `?page=1&limit=20`        |
| `GET`  | `/api/matches/:id`        | One own match                                        |
| `GET`  | `/health`, `/health/live` | Readiness and liveness; no session required          |

Pagination is validated, not clamped: `page` must be ≥ 1 and `limit` must be
1–100, otherwise the request is a `400`. The response carries `page`, `limit`,
`total` and `total_pages`.

`steam_id` is serialized as a **string**: a SteamID64 does not fit in a
JavaScript number. `dota_account_id` is a plain number — it is 32-bit.

Planned (Phases 4–5):

```text
GET    /api/players/me/stats     aggregates + time-of-day
GET    /api/players/me/coach     profile, patterns, current focus
POST   /api/matches/:id/analyze  AI analysis (rate-limited)
```

### Errors

One envelope everywhere, including malformed paths, bad query strings,
unparseable bodies and unknown routes — custom extractors route every rejection
through the same type, so no serde or SQL text ever reaches a client:

```json
{ "error": { "code": "NOT_FOUND", "message": "Match not found." } }
```

| Code                     | Status | When                                          |
| ------------------------ | ------ | --------------------------------------------- |
| `BAD_REQUEST`            | 400    | Invalid path, query or pagination             |
| `UNAUTHENTICATED`        | 401    | No session, or an expired/unknown cookie      |
| `NOT_FOUND`              | 404    | Unknown route, or a match the caller does not own |
| `DOTA_ACCOUNT_NOT_LINKED`| 409    | Signed in, but no Dota identity is linked     |
| `RATE_LIMITED`           | 429    | Sync cooldown, or the provider throttling us  |
| `UPSTREAM_UNAVAILABLE`   | 502    | OpenDota unreachable or malformed             |
| `DATABASE_ERROR`         | 500    | Storage failure                               |

Asking for someone else's match returns **404, not 403**: whether an id exists
is not information another user is entitled to.

---

## How Steam login and Dota linking work

```text
Browser ──GET /auth/steam/login──▶ backend
                                     │ mints a nonce, sets a 10-minute cookie
                                     ▼
                              302 to steamcommunity.com
                                     │  user approves on Valve's site
                                     ▼
Browser ──GET /auth/steam/callback?openid.*──▶ backend
                                     │ 1. nonce must match the cookie
                                     │ 2. POST check_authentication to Steam
                                     │ 3. Steam answers is_valid:true
                                     ▼
                        users ← account, dota_players ← linked identity
                        sessions ← new row; browser gets an HttpOnly cookie
                                     ▼
                              302 to the frontend
```

Points that matter:

- **The assertion is verified with Steam.** The redirect parameters are
  attacker-controlled until Valve confirms its own signature over them, so
  `check_authentication` is not optional and `claimed_id` is pinned to
  `https://steamcommunity.com/openid/id/<digits>`.
- **The Dota account id is derived, not supplied.** It is
  `SteamID64 − 76561197960265728`, computed from the proven identity. There is
  deliberately no code path that parses a player id out of a request.
- **The session cookie is opaque and `HttpOnly`.** 256 bits of CSPRNG output;
  only its SHA-256 is stored, so a database leak yields no usable cookies.
- **A login nonce blocks login CSRF**, so nobody can walk a victim's browser
  through a sign-in the victim did not start.
- Login failures redirect to `FRONTEND_BASE_URL/?error=<code>` with a stable
  code (`login_expired`, `steam_rejected`, `steam_unavailable`,
  `no_dota_account`, `server_error`) — never an internal message.

### Synchronization

`POST /api/players/me/sync` runs one idempotent pass:

1. Refresh the Steam profile and rank from the provider (best effort — a
   failure here does not fail the sync).
2. List recent matches, capped at `SYNC_MATCH_LIMIT`.
3. Drop match ids already stored for this player, and any the provider listed
   twice.
4. Fetch full detail for what remains, four requests in flight.
5. Insert. `UNIQUE (dota_player_id, match_id)` backstops concurrent syncs.

Running it repeatedly never creates duplicates: the second pass reports
`new_matches: 0` and `duplicates_skipped: N`. A match-detail failure is not
fatal — the summary is stored with `detail_synced = false` and a later sync
completes it. Syncs are throttled per player by `SYNC_COOLDOWN_SECONDS`.

Match rows carry the full per-match fact set (hero, role estimate, result,
duration, KDA, GPM/XPM, last hits, denies, net worth, hero/tower damage,
healing, game mode, lobby type, party size, start time), which is what Phase 4
needs to compute deterministic metrics without a schema redesign.

---

## How the AI coaching pipeline works

1. **Normalize.** The provider converts OpenDota payloads into domain models.
   Each sync lists recent matches, drops the ones already stored, fetches full
   detail for the rest (four requests in flight), and inserts. Duplicates are
   impossible: the planner diffs against stored ids and
   `UNIQUE (user_id, match_id)` backstops concurrent syncs. A match-detail
   failure is not fatal — the summary is stored with `detail_synced = false`
   and a later sync can complete it.
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

### Why roles are estimates

Dota does not publish the position a player took. OpenDota reports `lane_role`
only for **parsed** replays, which most public matches are not, so the role is
derived in two tiers:

1. **Parsed match** — lane assignment plus creep score per minute separates
   core from support inside a lane: `Carry`, `Mid`, `Offlane`, `Support`,
   `Hard Support`, `Roamer`, `Jungle`.
2. **Unparsed match** — net worth rank within the player's own five decides
   `Core` (top three), `Support` (fourth) or `Hard Support` (fifth). Lane is
   unknowable here, so no lane-specific label is invented.

With neither signal the role is `Unknown` rather than a guess. The UI presents
all of these as estimates.

---

## Testing

```bash
# Backend. The integration suite needs Postgres; unit tests do not.
docker compose up -d postgres
cd backend
DATABASE_URL='postgres://dota:dota@localhost:5432/dota_coach' cargo test

# Frontend
cd frontend && npm test
```

Nothing in the suite touches OpenDota or Valve:

- **Unit tests** cover provider normalization (from recorded OpenDota payloads
  in `backend/tests/fixtures/`), role estimation, the sync planner, session
  token hashing, cookie flags, OpenID claimed-id parsing, pagination validation
  and error mapping. They need neither a database nor a network.
- **Integration tests** (`backend/tests/api.rs`) drive the *real* router with a
  mock `DotaDataProvider` and a stub `SteamVerifier`, against a real Postgres.
  They cover the login round trip, rejection of anonymous, forged and expired
  sessions, login-nonce mismatches, sync idempotency, duplicate match ids,
  provider outages/rate limits/bad responses, pagination, and that one user
  cannot read another's matches.

Without `DATABASE_URL` (or `TEST_DATABASE_URL`) the integration tests print
`SKIPPED <name>` rather than passing silently.

---

## Security notes

- **Identity is server-side only.** The Steam id is written from a verified
  OpenID assertion; the Dota account id is derived from it arithmetically.
  There is no constructor anywhere that parses a player id from client input.
- **Ownership is enforced in SQL**, not by a check after loading: match reads
  are `WHERE id = $1 AND dota_player_id = $2`, so a wrong owner cannot be
  fetched at all.
- **Sessions** are 256-bit CSPRNG tokens in an `HttpOnly`, `SameSite=Lax`
  cookie; only the SHA-256 is stored. Set `COOKIE_SECURE=true` on HTTPS.
- **Credentials never reach the browser.** `LLM_API_KEY`, `DOTA_API_KEY` and
  the database URL live in backend config; only `NEXT_PUBLIC_*` is bundled.
- **Provider responses are validated** into typed domain models before storage;
  unknown or null fields become `None` rather than defaults that look real.
- **Internal errors are never serialized** — provider messages, SQL and stack
  detail are logged, and the client gets a fixed, safe message.

---

## Known limitations

- **One Dota account per user.** Enforced by `UNIQUE (user_id)` on
  `dota_players`; dropping that constraint is all a multi-account feature needs.
- **Steam profile comes from OpenDota**, not the Steam Web API, so a brand-new
  account shows no persona until the first sync. Avoiding a second provider and
  a second API key was the tradeoff.
- **`role` is an estimate.** See [Why roles are estimates](#why-roles-are-estimates).
- **Sync is synchronous.** A 20-match sync holds the request open for a few
  seconds; there is no job queue yet.
- **`SYNC_MATCH_LIMIT` caps history at 100.** There is no backfill of a full
  career.
- **Integration tests share one database** and clean up after themselves, so a
  test that fails mid-way can leave rows behind.
- **`npm run lint` is broken** by an ESLint 9 / `eslint-config-next` flat-config
  incompatibility that predates this phase. `npm run typecheck` and `npm test`
  both work.

---

## Roadmap

| Phase | Scope                                                         | Status |
| ----- | ------------------------------------------------------------- | ------ |
| 1     | Repo structure, Axum API, Postgres, Docker, basic frontend     | ✅ done |
| 2     | Dota provider, match sync, schema                              | ✅ done |
| 3     | Steam login, sessions, Dota linking, match history UI          | ✅ done |
| 4     | Deterministic metrics, dashboard, match page, charts           | next   |
| 5     | LLM abstraction, structured match analysis, analysis UI        |        |
| 6     | Player history, recurring patterns, profile, training focus    |        |
| 7     | PWA, landing polish, error states                              |        |

Deliberately out of scope: live coaching, overlays, voice, replay parsing,
native apps, social features, payments, leaderboards.
