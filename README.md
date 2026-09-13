# Dota Coach

A long-term personal AI coach for Dota 2. It reads your matches, turns them into
reproducible numbers, benchmarks you against comparable players, learns the
habits that repeat, and gives you **one** thing to train next — not a wall of
statistics.

The product answers:

- What am I good at, and what am I bad at?
- What mistakes keep repeating?
- Which heroes fit me, and which are strong for my rank and role right now?
- What should I work on next — and is it actually improving?

> Status: **Phase 5 complete.** Sign in with Steam, the backend resolves your
> Dota account, syncs matches into Postgres, computes a deterministic
> version-stamped metrics layer, and benchmarks you against the peer
> distribution for each hero — with percentiles withheld when the sample is too
> thin to support one. Hero intelligence and the AI coach follow — see
> [Roadmap](#roadmap).

Engineering rules that hold everywhere in this repo:

1. **The backend computes; the model interprets.** No statistic, percentile or
   entitlement is ever produced by an LLM.
2. **Providers are replaceable.** OpenDota, Valve's OpenID and any future
   STRATZ/payment provider sit behind traits; the domain never imports them.
3. **Identity comes from the session.** No endpoint accepts a user, player or
   Steam id from the caller.
4. **Missing data is represented, never invented.** An unavailable metric is
   `null` with its sample size, not a plausible-looking number.

---

## Product overview

Most Dota tools are dashboards: they show what happened. This is a coach — it
looks across many games and finds what repeats. A single bad fight is noise;
the same bad fight in four of the last ten games is a pattern, and a pattern is
something you can train.

Nothing here promises MMR gains, and every score shown is an estimate derived
from public match data.

---

## Architecture

```text
Steam OpenID           proves who the user is; nothing else may set identity
        │
        ▼
Dota data (OpenDota)   behind DotaDataProvider
        │
        ▼
Provider layer         normalizes payloads into internal domain models
        │
        ▼
Deterministic metrics  KDA, per-10 rates, participation, timings  [done]
        │
        ▼
Benchmark engine       percentile, top 20%, gap, confidence       [done]
        │
        ▼
Hero pool              signature / comfort / stretch / risk       [phase 6]
        │
        ▼
Hero intelligence      which strong heroes actually fit you       [phase 6]
        │
        ▼
Player model           strengths, weaknesses, recurring patterns  [phase 7-8]
        │
        ▼
AI coaching            LLM reasons over structured data only      [phase 7]
        │
        ▼
Training focus         exactly one current focus                  [phase 9]
        │
        ▼
Progress tracking      is the focus actually improving?           [phase 9]
```

Four rules hold the design together:

1. **The LLM never does arithmetic.** All numbers are computed in Rust, are
   reproducible, and are version-stamped. The model only interprets them.
2. **Providers are replaceable.** OpenDota response shapes stop at the provider
   boundary; everything above it speaks the internal domain model, so STRATZ (or
   anything else) can be swapped in without touching handlers.
3. **Identity comes from the session, always.** The Steam id is written only
   from a verified OpenID assertion, and every query is scoped to the caller's
   own player row. No endpoint accepts a user, player or Steam id as input.
4. **Derived values are version-stamped.** Every metric row records the formula
   set that produced it and is recomputed when either the formula or its inputs
   change, so a stored number always matches its current definition.

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
│       ├── domain/            # user, session, player, match, metrics
│       ├── services/
│       │   ├── auth/          # Steam OpenID + server-side sessions
│       │   ├── dota/          # DotaDataProvider trait + OpenDota impl
│       │   ├── sync/          # fetch -> dedupe -> enrich -> store -> compute
│       │   ├── metrics/       # deterministic metric engine (pure functions)
│       │   ├── benchmarks/    # BenchmarkProvider + percentile engine
│       │   ├── coaching/      # profile, patterns, training focus  [phase 7+]
│       │   └── llm/           # LlmProvider trait                  [phase 7]
│       ├── repositories/      # SQL access, one module per aggregate
│       └── tests/             # integration suite against the real router
└── frontend/
    ├── Dockerfile
    └── src/
        ├── app/               # App Router: /, /matches, /benchmark, /profile
        ├── components/        # shell / dashboard / matches / charts / ui
        └── lib/               # api client, types, hero map, formatters
```

Business logic lives in the backend. React components consume the API and
render; **they compute nothing**. The frontend holds formatters and chart
reshaping only — every average, rate and rollup arrives from `/api/stats`.

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
| `BENCHMARK_TTL_HOURS`    | backend  | How long a cached peer distribution stays fresh. Default 24.        |
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

| Method | Path                       | Description                                      |
| ------ | -------------------------- | ------------------------------------------------ |
| `GET`  | `/api/auth/steam`          | Redirects to Steam. Sets the login nonce cookie. |
| `GET`  | `/api/auth/steam/callback` | Steam returns here; sets the session cookie      |
| `GET`  | `/api/auth/me`             | The signed-in account, or `401`                  |
| `POST` | `/api/auth/logout`         | Destroys the session and clears the cookie       |

The two login endpoints are reached by **top-level browser navigation** and
answer with redirects, not JSON — OpenID cannot be completed from `fetch`.

### Player and matches

| Method | Path                      | Description                                        |
| ------ | ------------------------- | -------------------------------------------------- |
| `GET`  | `/api/players/me`         | Steam profile + linked Dota identity + match count |
| `POST` | `/api/players/me/sync`    | Fetch, dedupe, store, back-fill and compute        |
| `GET`  | `/api/matches`            | Own history, newest first. `?page=1&limit=20`      |
| `GET`  | `/api/matches/:id`        | One own match, with its derived KDA                |
| `GET`  | `/api/stats`              | Aggregates: overall, per hero, per role            |
| `GET`  | `/api/benchmark`          | Peer comparison. `?hero_id=` picks the hero        |
| `GET`  | `/api/benchmark/:metric`  | The same, narrowed to one metric                   |
| `GET`  | `/health`, `/health/live` | Readiness and liveness; no session required        |

Pagination is validated, not clamped: `page` must be ≥ 1 and `limit` must be
1–100, otherwise the request is a `400`. The response carries `page`, `limit`,
`total` and `total_pages`.

`steam_id` is serialized as a **string**: a SteamID64 does not fit in a
JavaScript number. `dota_account_id` is a plain number — it is 32-bit.

Planned, in roadmap order:

```text
GET    /api/heroes               hero pool                  phase 6
GET    /api/heroes/recommendations
GET    /api/hero-intelligence
POST   /api/matches/:id/analyze  AI analysis (rate-limited) phase 7
GET    /api/coach                insights, patterns         phase 7-8
GET    /api/coach/training-focus                            phase 9
GET    /api/billing              subscription + payments    phase 10
POST   /api/billing/checkout
POST   /api/billing/webhook
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
6. Back-fill a bounded batch of older matches with facts a later schema added.
7. Recompute deterministic metrics for anything missing, out-of-version or
   whose inputs changed.

Running it repeatedly never creates duplicates: the second pass reports
`new_matches: 0` and `duplicates_skipped: N`. A match-detail failure is not
fatal — the summary is stored with `detail_synced = false` and a later sync
completes it. Syncs are throttled per player by `SYNC_COOLDOWN_SECONDS`.

Match rows carry the full per-match fact set (hero, role estimate, result,
duration, KDA, GPM/XPM, last hits, denies, net worth, hero/tower damage,
healing, game mode, lobby type, party size, start time, team totals, and — for
parsed replays — time-sliced snapshots and item timings). The response reports
`metrics_computed` and `facts_backfilled` alongside the fetch counters.

---

## Deterministic metrics

Metrics are computed in Rust, from stored facts, by pure functions in
`services/metrics`. Nothing here calls a provider or a model, and running it
twice on the same row gives the same answer — which is precisely what lets a
later coaching layer *interpret* numbers it did not invent.

Two layers, deliberately separated:

| Table           | Holds                                            |
| --------------- | ------------------------------------------------ |
| `matches`       | raw facts, exactly as the provider reported them |
| `match_metrics` | values derived from those facts, version-stamped |

### What is computed

Always available, from any synced match:

```text
KDA                  (kills + assists) / max(deaths, 1)
Kills / 10 min       rate over game length
Deaths / 10 min
Assists / 10 min
Last hits per minute
Hero damage per minute
Tower damage per minute
Kill participation   (kills + assists) / team kills
```

Available **only for parsed replays**, which most public matches are not:

```text
Last hits @10, @15
Net worth  @10, @15
XP         @10, @15
BKB / Blink / Midas timing   first purchase, seconds from the horn
Teamfight participation
```

`GET /api/stats` reports `parsed_matches` alongside `matches` so a client can
say *why* a timing metric is absent, and `kill_participation_sample` alongside
the average so a figure built on three games is never presented as though it
were built on thirty. A missing metric is `null` — never a zero, and never a
plausible-looking guess.

### Recomputation

A metrics row is rebuilt when it is missing, when `METRICS_VERSION` moves, or
when its inputs changed (`match_metrics.computed_at < matches.updated_at`).
That last condition matters: a fact back-fill rewrites the match row, leaving a
metrics row that is the right *version* but the wrong *answer*.

Because deduplication means a stored match is never re-fetched, each sync also
back-fills a bounded batch of older matches with facts a later schema added —
otherwise a new column would only ever populate for matches synced after it
landed.

---

## Benchmarking

The engine answers one question: **how does this player compare to appropriate
players?** It owns the arithmetic and the honesty rules; a `BenchmarkProvider`
only supplies a distribution, so STRATZ can replace OpenDota later without the
percentile, confidence or gap logic moving.

### What the current provider can and cannot do

`GET /benchmarks?hero_id=N` — verified against the live endpoint before the
provider was written — returns 11 percentile buckets (p0.1 … p0.99) across 10
metrics, **segmented by hero only**. No rank bracket, no role, no patch, and no
sample size for the peer group.

That is a real limitation, not a temporary gap, so it is reported rather than
hidden. Every response carries `segmented_by: ["hero"]` and a null
`peer_sample_size`, and the UI says in plain words that these are percentiles
against *everyone who plays this hero*, not against players of the same rank.
Claiming rank-awareness the data cannot support would be exactly the
fabrication the spec forbids.

### The rules the engine enforces

- **No percentile below the sample floor.** Under five matches on a hero, the
  player's own value and the reference distribution are both shown, but no rank
  is asserted — an average over three games measures variance, not skill.
  Between five and fifteen the figure is returned with `confidence: "low"`.
- **Direction is honoured.** Sitting in the 90th percentile for *deaths* is a
  bad result. Percentiles are inverted for less-is-better metrics, so 90 always
  means "better than 90% of players", whichever row you are reading.
- **Gaps are signed consistently.** `gap_to_top_20` is positive whenever there
  is work to do, in both directions, so a client never has to know which way a
  metric runs to render it.
- **Values outside the reported range clamp to its edges** rather than
  extrapolating a percentile the provider never measured.
- **A missing metric is omitted, not defaulted.** No distribution means no
  percentile and an explanatory note, never a zero.
- **A provider outage degrades the page.** The player's own figures are local;
  they are still shown, with the comparison marked unavailable.

Distributions are cached in `benchmark_snapshots` (Postgres, `BENCHMARK_TTL_HOURS`,
default 24). They are identical for every user, so they are fetched once, shared,
and survive a restart rather than costing each deploy a fresh stampede.

---

## How the AI coaching pipeline will work

Phases 6-9 build on the metrics and benchmark layers. The shape is fixed even
where the code is not yet written:

1. **Normalize.** The provider converts payloads into domain models. Duplicates
   are impossible: the planner diffs stored ids and `UNIQUE (dota_player_id,
   match_id)` backstops concurrent syncs.
2. **Compute.** Deterministic metrics, above.
3. **Benchmark.** Done — see above. Currently hero-scoped; rank and role
   segmentation wait on a provider that offers them.
4. **Aggregate.** Hero pool and player model, evolving as matches arrive rather
   than being rebuilt per match.
5. **Analyse.** The LLM receives a compact structured payload — never raw API
   blobs, never user-controlled prompt text — and must return strict JSON that
   is validated before anything is stored.
6. **Focus.** One training focus at a time, chosen from benchmark gap, pattern
   history, recency, impact and confidence — not simply the lowest statistic.

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
  in `backend/tests/fixtures/`), the metric formulas (KDA, per-10 rates, kill
  participation, series indexing, item timings), role estimation, the sync
  planner, percentile interpolation and direction, sample-size gating, provider
  payload parsing (from a recorded `/benchmarks` response), session token
  hashing, cookie flags, OpenID claimed-id parsing, pagination validation and
  error mapping. No database, no network.
- **Integration tests** (`backend/tests/api.rs`) drive the *real* router with a
  mock `DotaDataProvider` and a stub `SteamVerifier`, against a real Postgres.
  They cover the login round trip, rejection of anonymous, forged and expired
  sessions, login-nonce mismatches, sync idempotency, duplicate match ids,
  provider outages/rate limits/bad responses, pagination, metric computation
  and invalidation, benchmark ranking and its refusal to rank a thin sample,
  and that one user can read neither another's matches nor another's
  statistics.

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
- **Analytics are session-scoped.** `/api/stats` aggregates only the caller's
  own player row; there is no parameter that could widen it.
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
- **Sync is synchronous.** A sync holds the request open for a few seconds;
  there is no job queue yet. Phase 7's per-match LLM calls will need one.
- **Time-sliced metrics need a parsed replay.** Last hits at 10, net worth at
  15 and item timings exist only where OpenDota parsed the replay, which is a
  minority of public matches. They are reported as `null`, with
  `parsed_matches` alongside so the UI can explain the gap.
- **Benchmarks are hero-scoped, not rank-scoped.** OpenDota's distribution
  covers every rank playing that hero. A rank-aware comparison needs STRATZ,
  which the `BenchmarkProvider` trait is shaped for but which is not wired up.
- **Peer sample sizes are unknown.** The provider does not publish them, so
  `peer_sample_size` is always `null` rather than a guess.
- **`SYNC_MATCH_LIMIT` caps history at 100.** There is no backfill of a full
  career.
- **Integration tests share one database** and clean up after themselves, so a
  test that fails mid-way can leave rows behind.
- **`npm run lint` is broken** by an ESLint 9 / `eslint-config-next` flat-config
  incompatibility that predates this phase. `npm run typecheck` and `npm test`
  both work.

---

## Roadmap

Phases follow `PRODUCT_SPEC.md`.

| Phase | Scope                                                        | Status  |
| ----- | ------------------------------------------------------------ | ------- |
| 1     | Repository assessment, Axum API, Postgres, Docker, frontend    | ✅ done |
| 2     | Steam OpenID, users, sessions, auth middleware                 | ✅ done |
| 3     | DotaProvider, player resolution, match sync and persistence    | ✅ done |
| 4     | Deterministic metrics, player/hero/role statistics             | ✅ done |
| 5     | Benchmark engine: percentiles, top 20%, sample validation      | ✅ done |
| 6     | Hero intelligence: meta providers, hero pool, fit score        | next    |
| 7     | AI coach: LLM provider, evidence-based insights                |         |
| 8     | Player model and recurring pattern detection                   |         |
| 9     | Training focus and progress tracking                           |         |
| 10    | Trial, entitlements, crypto billing, webhooks                  |         |
| 11    | PWA, landing page, production configuration, observability     |         |

Deliberately **out of scope** until the core loop is excellent: microservices,
live overlay, voice coaching, replay parsing, native apps, social features,
leaderboards, full draft assistant.
