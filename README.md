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

> Status: **Phase 11 complete — the roadmap is done.** Sign in with Steam, the
> backend resolves your Dota account, syncs matches into Postgres, computes a deterministic
> version-stamped metrics layer, benchmarks you against the peer distribution
> for each hero — with percentiles withheld when the sample is too thin to
> support one — scores which currently-strong heroes actually fit *you*, keeps
> a long-term model of your habits that only calls something recurring once it
> has the evidence, picks **one** training focus with a checkable target and
> tracks whether it is actually improving, and has an LLM interpret all of it
> into insights it is **not allowed to make numbers up in** — free for fourteen
> days, then $9.98/month for the AI coaching, settled in crypto and activated only
> by a payment the provider signed for. It installs as a PWA, says something
> useful when it is offline, and tags every request with an id you can quote.

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
Hero pool              signature / comfort / stretch / risk       [done]
        │
        ▼
Hero intelligence      which strong heroes actually fit you       [done]
        │
        ▼
Player model           strengths, weaknesses, recurring patterns  [done]
        │
        ▼
AI coaching            LLM reasons over structured data only      [done]
        │
        ▼
Training focus         exactly one current focus                  [done]
        │
        ▼
Progress tracking      is the focus actually improving?           [done]
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
│       │   ├── extract.rs     # CurrentUser/EntitledUser + JSON rejections
│       │   ├── observability.rs # request ids and one log line per request
│       │   └── handlers/
│       ├── domain/            # user, session, player, match, metrics, hero,
│       │                       #   coaching, player_model, training, admin,
│       │                       #   event, voucher, audit
│       ├── services/
│       │   ├── auth/          # Steam OpenID + server-side sessions
│       │   ├── dota/          # DotaDataProvider trait + OpenDota impl
│       │   ├── sync/          # fetch -> dedupe -> enrich -> store -> compute
│       │   ├── metrics/       # deterministic metric engine (pure functions)
│       │   ├── benchmarks/    # BenchmarkProvider + percentile engine
│       │   ├── hero_meta/     # HeroMetaProvider + meta strength scoring
│       │   ├── heroes/        # hero pool + fit score (pure functions)
│       │   ├── player_model/  # pattern detectors + long-term model
│       │   ├── training/      # focus selection, goals, progress series
│       │   ├── coaching/      # evidence builder, prompt, answer validation
│       │   ├── billing/       # trial, entitlement, idempotent settlement
│       │   ├── payments/      # PaymentProvider trait + NOWPayments impl
│       │   ├── llm/           # LlmProvider trait + OpenAI-compatible impl
│       │   ├── admin/         # GET /admin/stats assembly
│       │   ├── events/        # trackEvent-equivalent, fire-and-forget
│       │   ├── voucher/       # redemption transaction + rate limiter
│       │   └── audit/         # admin-action logging, fire-and-forget
│       ├── repositories/      # SQL access, one module per aggregate
│       ├── bin/                # backfill_subscriptions — one-off, idempotent
│       └── tests/             # integration suite against the real router
└── frontend/
    ├── Dockerfile
    └── src/
        ├── app/               # App Router: /, /matches, /benchmark, /heroes,
        │                      #   /coach, /billing, /profile, /offline,
        │                      #   /admin, /admin/users, /admin/vouchers
        │                      #   + manifest, error, not-found, loading states
        ├── components/        # shell / dashboard / matches / heroes / coach /
        │                      #   billing / charts / ui / admin
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
| `COOKIE_CROSS_SITE`      | backend  | `true` when the frontend is on another site: `SameSite=None`. Needs `COOKIE_SECURE`. |
| `RUST_LOG`               | backend  | Tracing filter.                                                     |
| `DOTA_API_BASE_URL`      | backend  | Defaults to OpenDota.                                               |
| `DOTA_API_KEY`           | backend  | Optional. Raises OpenDota's rate limit.                             |
| `SYNC_MATCH_LIMIT`       | backend  | Matches pulled per sync, 1-100. Default 20.                         |
| `SYNC_COOLDOWN_SECONDS`  | backend  | Per-player sync throttle. Default 30; `0` disables.                 |
| `BENCHMARK_TTL_HOURS`    | backend  | How long a cached peer distribution stays fresh. Default 24.        |
| `HERO_META_TTL_HOURS`    | backend  | How long a cached hero meta cohort stays fresh. Default 24.         |
| `FOCUS_WEIGHT_GAP`       | backend  | Focus weight: benchmark gap. Default 0.25.                          |
| `FOCUS_WEIGHT_PATTERN`   | backend  | Focus weight: historical pattern. Default 0.20.                     |
| `FOCUS_WEIGHT_RECENT`    | backend  | Focus weight: recent performance. Default 0.15.                     |
| `FOCUS_WEIGHT_IMPACT`    | backend  | Focus weight: impact. Default 0.20.                                 |
| `FOCUS_WEIGHT_CONFIDENCE`| backend  | Focus weight: confidence. Default 0.10.                             |
| `FOCUS_WEIGHT_RECENCY`   | backend  | Focus weight: recency. Default 0.10.                                |
| `FOCUS_HISTORY_LIMIT`    | backend  | Past focuses returned with the current one. Default 10.             |
| `COACH_COOLDOWN_SECONDS` | backend  | Minimum gap between two generations per player. Default 30.         |
| `COACH_DAILY_LIMIT`      | backend  | Generations per player per rolling 24h. Default 20; `0` disables.   |
| `COACH_MAX_INSIGHTS`     | backend  | Insights kept from one answer. Default 5.                           |
| `COACH_MAX_OUTPUT_TOKENS`| backend  | Output ceiling for one model call. Default 900.                     |
| `COACH_TEMPERATURE`      | backend  | Default 0.2 — interpretation, not creative writing.                 |
| `COACH_RECENT_MATCHES`   | backend  | Matches feeding the recent-form evidence. Default 10.               |
| `LLM_TIMEOUT_SECONDS`    | backend  | How long one model call may take. Default 30.                       |
| `HERO_RECOMMENDATION_LIMIT` | backend | Candidates returned by default, 1-50. Default 8.                  |
| `HERO_BENCHMARK_LOOKUPS` | backend  | Peer distributions fetched per recommendation request. Default 5.   |
| `FIT_WEIGHT_PERFORMANCE` | backend  | Fit weight: your performance. Default 0.30.                         |
| `FIT_WEIGHT_META`        | backend  | Fit weight: meta strength. Default 0.25.                            |
| `FIT_WEIGHT_EXPERIENCE`  | backend  | Fit weight: experience. Default 0.20.                               |
| `FIT_WEIGHT_BENCHMARK`   | backend  | Fit weight: benchmark. Default 0.15.                                |
| `FIT_WEIGHT_RECENT_FORM` | backend  | Fit weight: recent form. Default 0.10.                              |
| `LLM_BASE_URL`           | backend  | Any OpenAI-compatible base URL.                                     |
| `LLM_API_KEY`            | backend  | **Server-side only.** Never prefixed with `NEXT_PUBLIC_`.           |
| `LLM_MODEL`              | backend  | Model identifier.                                                   |
| `BILLING_PLAN`           | backend  | Plan name stored on the subscription. Default `pro`.                |
| `BILLING_PRICE_CENTS`    | backend  | The price, in minor units. Default 100 ($1). Must be > 0.           |
| `BILLING_CURRENCY`       | backend  | ISO-4217 the price is denominated in. Default `usd`.                |
| `BILLING_TRIAL_DAYS`     | backend  | Trial length, anchored to account creation. Default 14.             |
| `BILLING_PERIOD_DAYS`    | backend  | Length of one paid period. Default 30.                              |
| `BILLING_HISTORY_LIMIT`  | backend  | Charges returned by the billing endpoints. Default 20.              |
| `BILLING_ENFORCE`        | backend  | Whether an expired account loses AI coaching. Defaults to on only when a payment provider is configured. |
| `NOWPAYMENTS_BASE_URL`   | backend  | Defaults to NOWPayments.                                            |
| `NOWPAYMENTS_API_KEY`    | backend  | **Server-side only.** Without it, checkout answers `503`.           |
| `NOWPAYMENTS_IPN_SECRET` | backend  | **Server-side only.** Verifies callbacks; required for checkout.    |
| `NOWPAYMENTS_PAY_CURRENCY` | backend | Restrict checkout to one coin. Unset lets the payer choose.        |
| `BILLING_TIMEOUT_SECONDS`| backend  | How long one payment-provider call may take. Default 15.            |
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
| `GET`  | `/api/matches`            | Own history, newest first. `?page=1&limit=20&scope=all\|competitive` |
| `GET`  | `/api/matches/:id`        | One own match, with its derived KDA                |
| `GET`  | `/api/stats`              | Overall analysis over the competitive window: scope, eligibility, per hero, per role |
| `GET`  | `/api/benchmark`          | Peer comparison. `?hero_id=` picks the hero, `?role=` the scope |
| `GET`  | `/api/benchmark/:metric`  | The same, narrowed to one metric                   |
| `GET`  | `/api/heroes`             | Your hero pool. No provider call — always answers  |
| `GET`  | `/api/heroes/recommendations` | Scored candidates, best fit first. `?limit=`   |
| `GET`  | `/api/hero-intelligence`  | Pool, meta and recommendations in one payload      |
| `GET`  | `/api/coach`              | Measured evidence + the last analysis. No model call |
| `GET`  | `/api/coach/roles`        | Role performance, the advisory pick, the current choice |
| `POST` | `/api/coach/role`         | Choose the role to be coached on. Any of the five   |
| `POST` | `/api/coach/analyze`      | Generates one. Rate limited; the only paid call    |
| `GET`  | `/api/coach/player-model` | Traits, role affinity and recurring patterns       |
| `GET`  | `/api/coach/training-focus` | The one focus, its progress, and the runners-up  |
| `GET`  | `/api/matches/:id/analysis` | The stored analysis for one match, if any        |
| `POST` | `/api/matches/:id/analyze`  | Generates one for that match                     |
| `GET`  | `/health`, `/health/live` | Readiness and liveness; no session required        |

### Billing

| Method | Path                        | Description                                      |
| ------ | --------------------------- | ------------------------------------------------ |
| `GET`  | `/api/billing/plan`         | The offer alone. **No session** — the landing page quotes it |
| `GET`  | `/api/billing`              | Entitlement, subscription, plan and charges      |
| `GET`  | `/api/billing/subscription` | The entitlement on its own                       |
| `GET`  | `/api/billing/payments`     | This account's charges, newest first             |
| `POST` | `/api/billing/checkout`     | Opens a charge, or returns the one still open    |
| `POST` | `/api/billing/webhook`      | Provider notification. Signed, not authenticated |

Every billing read is derived server-side from stored timestamps. The webhook is
the only unauthenticated write in the API, and the only request that can grant
access — see [Billing](#billing-trial-subscription-and-settlement).

Pagination is validated, not clamped: `page` must be ≥ 1 and `limit` must be
1–100, otherwise the request is a `400`. The response carries `page`, `limit`,
`total` and `total_pages`.

`steam_id` is serialized as a **string**: a SteamID64 does not fit in a
JavaScript number. `dota_account_id` is a plain number — it is 32-bit.

### Vouchers

| Method | Path                    | Description                                      |
| ------ | ----------------------- | ------------------------------------------------ |
| `POST` | `/api/subscribe/redeem` | Redeem a code for subscription time. Any signed-in account — existing access is not a precondition |

See [Admin panel and vouchers](#admin-panel-and-vouchers).

### Admin

Every route below requires `users.is_admin` — see
[Admin panel and vouchers](#admin-panel-and-vouchers). There is no self-serve
promotion path; the first admin is set directly in the database.

| Method | Path                                  | Description                                      |
| ------ | -------------------------------------- | ------------------------------------------------ |
| `GET`  | `/api/admin/stats`                     | Usage, trial, funnel and revenue counts. `?from&to`, default last 30 days |
| `GET`  | `/api/admin/users`                     | Paginated, filtered by status/plan/last-login, searched by persona name or SteamID64 |
| `GET`  | `/api/admin/users/:id`                 | Profile, subscription and event timeline         |
| `POST` | `/api/admin/users/:id/extend`          | Extends whichever window governs access — trial end or paid-through date |
| `POST` | `/api/admin/users/:id/disable`         | Every future request from this account is refused |
| `POST` | `/api/admin/users/:id/enable`          | Reverses a disable                               |
| `POST` | `/api/admin/vouchers`                  | Create one voucher, or `count` independent ones sharing the same settings |
| `GET`  | `/api/admin/vouchers`                  | Paginated, with usage counts                     |
| `GET`  | `/api/admin/vouchers/:id`              | One voucher and everyone who has redeemed it     |
| `POST` | `/api/admin/vouchers/:id/deactivate`   | Existing redemptions are unaffected — only future ones are refused |
| `GET`  | `/api/admin/audit-log`                 | Every mutating admin action above, newest first  |

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
| `ACCOUNT_DISABLED`       | 403    | The session is valid; the account itself has been disabled by an admin |
| `FORBIDDEN`              | 403    | Signed in, but `users.is_admin` is false — the `/api/admin/*` gate |
| `NOT_FOUND`              | 404    | Unknown route, or a match the caller does not own |
| `DOTA_ACCOUNT_NOT_LINKED`| 409    | Signed in, but no Dota identity is linked     |
| `PRECONDITION_UNMET`     | 409    | Nothing to analyse yet — no synced matches    |
| `RATE_LIMITED`           | 429    | Sync or coaching cooldown, or a provider throttling us |
| `FEATURE_UNAVAILABLE`    | 503    | The feature is not configured on this deployment (no `LLM_API_KEY`) |
| `UPSTREAM_UNAVAILABLE`   | 502    | OpenDota or the coaching model unreachable, or its answer unusable |
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

## How hero intelligence works

The question is **not** "which hero has the highest win rate". It is:

> Among the heroes that are currently strong, which ones are actually a good
> fit for *me*?

Three inputs, combined deterministically in Rust:

```text
HeroMetaProvider ──► meta strength   what the ladder is doing
matches + metrics ─► hero pool       what you have actually done
benchmark engine ──► percentile      how you compare on each hero
                        │
                        ▼
                  Hero Fit Score ──► Recommended / Consider / Avoid for now
```

### Meta strength is not the win rate

Dota win rates live between roughly 45% and 55%, so ordering by win rate alone
is both unreadable and wrong — a 52% hero nobody picks and a 52% hero a quarter
of the ladder picks are not equally strong. `meta_strength` is a 0-100 score
over three signals the provider genuinely publishes:

- **Win rate**, measured against the spread of the whole cohort rather than
  against 50%.
- **Pick rate**, by *rank* within the cohort — pick rates are long-tailed, and
  a linear scale would flatten the entire middle of the roster.
- **Recent trend**, when the provider publishes one.

Heroes with few recorded picks are pulled toward the neutral midpoint in
proportion to how short they fall, so 50 games at 70% never outranks 10,000 at
55%. Ban rate is deliberately **absent**: the only ban figure OpenDota publishes
comes from professional matches, a different population from the pubs this
product coaches.

### The hero pool classifies from your history, not from opinion

| Tier      | Meaning                                                      |
| --------- | ------------------------------------------------------------ |
| Signature | 10+ matches, win rate clearly above **your own** average      |
| Comfort   | 5+ matches, results around your average                       |
| Stretch   | Fewer than 5 matches — not enough evidence to judge yet       |
| Risk      | 5+ matches, results clearly below your average                |

Relative to the player, not to 50%: a 48% hero is a strength for a 42% player
and a weakness for a 55% one. A 3-0 hero is `Stretch`, because three games
cannot tell a signature hero from a lucky streak.

### The fit score explains itself

Five weighted components, defaults from `PRODUCT_SPEC.md` §19 and all
overridable by environment variable:

| Component        | Default weight | Source                                     |
| ---------------- | -------------- | ------------------------------------------ |
| Your performance | 30%            | hero win rate and KDA vs your own baseline  |
| Meta strength    | 25%            | `HeroMetaProvider`                          |
| Experience       | 20%            | matches on the hero, square-rooted          |
| Benchmark        | 15%            | mean percentile from the benchmark engine   |
| Recent form      | 10%            | your last 10 matches **on that hero**       |

A component whose input is *unknowable* — meta down, no peer distribution — is
dropped and the remaining weights are renormalized, so an outage lowers
confidence rather than the score. Every response reports the weights that
actually produced the total, plus a one-line reason per component.

Zero experience is not unknowable, it is knowledge: an unplayed hero scores 0
there and is weighted normally. Two rules then override the score itself,
because the spec forbids the meta from overriding coaching context:

- Under five matches, a hero is **never** a full recommendation, however strong
  the meta says it is.
- Five or more recent matches below a 35% win rate demote a hero regardless of
  its lifetime record.

### Providers

Spec priority is STRATZ, then OpenDota, then an optional Dotabuff. Only
OpenDota is implemented. STRATZ's GraphQL API answers `403` to unauthenticated
requests, so its response shape cannot be verified without a token, and
inventing fields is exactly what the provider rules forbid — `HeroMetaProvider`
is the seam it drops into once one exists. Dotabuff is not a dependency and no
HTML is scraped.

OpenDota's `/heroStats` **does** segment by rank bracket, so unlike the
benchmark endpoint, hero meta is genuinely rank-aware and reports
`segmented_by: ["rank_bracket"]`. Two honest limits are surfaced rather than
hidden: the trend arrays are published across all brackets, and the Immortal
columns are currently empty — asking for Immortal falls back to all brackets
and says so. Cohorts are cached in `hero_meta_snapshots` (`HERO_META_TTL_HOURS`,
default 24).

There are no `hero_pool` or `hero_recommendations` tables. Both are pure
functions of `matches`, `match_metrics` and the meta snapshot; persisting them
would only create a second, staler answer.
---

## How the player model works

The spec's framing: a new user gets generic analysis, a player with a hundred
matches gets something personal. The model is the thing that makes the second
possible, and the confidence dial is what stops the first pretending to be it.

```text
matches + metrics ──► detectors ──► patterns ──► stored (first seen, resolved)
                                        │
benchmarks + hero pool + roles ─────────┴──────► player model
```

### A pattern needs evidence, and evidence has a denominator

Every detector answers one question per match with **three** possible answers:
yes, no, or *not measurable here*. The third is what makes this honest — last
hits at ten minutes only exist on a parsed replay, so a detector that read a
missing value as "fine" would report a clean laning phase for a player nobody
ever measured.

A pattern is reported only when it clears three separate bars:

| Bar               | Value | Why                                             |
| ----------------- | ----- | ----------------------------------------------- |
| Measurable in     | 8     | Fewer is a sample, not a history                |
| Occurrences       | 3     | 3 of 3 is a 100% rate and no evidence           |
| Rate              | 40%   | 8 of 40 is a solid sample of something rare     |

All three matter independently. Each stored pattern carries both numbers, and
the sentence the UI and the coach share states them: *"Dies too often in 12 of
the 20 matches this could be measured in (60%)."* Detectors that stayed silent
are listed with the count they managed, because silence would otherwise read as
a pass.

The detectors: death rate, teamfight participation, farming well while absent
from fights, laning stage (cores), losing a lane you won, late Black King Bar,
and objective damage. Thresholds are documented constants — the line between
"worth mentioning" and "not", not a skill rating. Grading against real players
is the benchmark engine's job.

### Why any of it is stored

Almost all of the model is derived and recomputed on read: strengths and
weaknesses from benchmark percentiles, role affinity, form, hero confidence.
Two things about a pattern cannot be:

- **When it was first noticed.** `first_detected_at` survives every
  recomputation, which is what lets the coach say how long something has been
  true.
- **That it used to be true.** A resolved pattern is invisible in the data that
  resolved it — the whole point is that it no longer happens. Deleting the row
  would erase the fact that a player fixed something.

So a pattern that stops clearing the threshold is marked `resolved` rather than
dropped, and one that comes back has its resolution cleared. A pattern that is
still present overall but markedly rarer lately is `improving`, which is a
different claim from either.

### Confidence

| Matches | Confidence   | What it means                                   |
| ------- | ------------ | ----------------------------------------------- |
| < 10    | `sparse`     | A first impression, not a model                 |
| 10–29   | `developing` | Enough to see trends, not to be sure of them    |
| 30+     | `established`| Enough history for the claims to carry weight   |

Patterns feed the coach as evidence with ids like `pattern.high_death_rate`, so
an insight of kind `recurring_pattern` is pinned to a measured rate with a real
denominator instead of extrapolating a habit from an average. Detection runs on
every sync and on every read of the model; it is arithmetic over stored rows
and never touches a provider.

---

## How the training focus works

The product's question is "what should I do next?", and the spec is blunt about
both halves of the answer: **one** focus at a time, and **not** simply the
lowest statistic.

```text
recurring patterns ─┐
                    ├─► candidates ─► score ─► one focus ─► progress series
benchmark gaps ─────┘                               │
                                                    └─► hero fit modifier
```

### Selection weighs six things, and says so

Each candidate is scored on the inputs the spec names, and the parts come back
with the total so "why this one" is answerable:

| Input               | Default weight | What it reads                          |
| ------------------- | -------------- | -------------------------------------- |
| Benchmark gap       | 25%            | How far below the peer distribution     |
| Historical pattern  | 20%            | Whether a detected pattern agrees       |
| Recent performance  | 15%            | Whether it is getting worse             |
| Impact              | 20%            | How much moving it tends to change games |
| Confidence          | 10%            | The sample behind the figure            |
| Recency             | 10%            | How recently it was observed            |

All six are `FOCUS_WEIGHT_*` environment variables. Impact is a stated
judgement rather than a derived number — dying less changes more games than
last-hitting slightly faster — and it lives in one table in the domain rather
than smeared through a scoring expression.

The effect of weighing them together is that a spectacular benchmark gap on a
low-impact measure loses to a moderate, corroborated pattern on a high-impact
one. That is the point.

### A focus is a promise with a number on it

Every focus carries a **measure**, a **baseline**, a **target** and a
direction, because "is this improving?" has to be arithmetic:

```text
Dies too often
  when set   5.0 deaths per 10 minutes
  now        3.2
  target     2.0          ← the peer median, not an aspiration
  progress   60%
```

Targets are chosen to be reachable: the peer median first, and only the top-20%
line for a player already past it. A pattern's target is set comfortably below
the rate at which the detector flags it in the first place.

The progress series buckets the player's history into fixed ten-match slices
rather than calendar weeks — a player who plays twice one week and thirty times
the next would otherwise get two points of wildly different weight plotted as
equals. A bucket nobody can measure is omitted, never zeroed.

### Stability, and knowing when to stop

The chosen focus is **stored**, for two reasons a recompute-on-read design
would lose:

- A focus recomputed on every request would change whenever a match landed.
  That is a feed, not a training plan.
- Progress is measured against where the player stood *when the focus was set*,
  and that baseline only exists if it was captured at the time.

It is replaced only when it is finished or its evidence has gone:

- **Achieved** — the target is met across a full recent window. The finished
  focus is excluded from the next selection, so a player is never handed back
  the goal they just met.
- **Retired** — the pattern behind it stopped being detected. Not claimed as a
  success, because it was not one.

`UNIQUE … WHERE status = 'active'` enforces one focus per player in the
database, rather than trusting the code that writes it.

### Where it shows up

The focus leads the dashboard and the coach page, it is fed to the LLM as
`focus.current` evidence so the advice points at the same thing the product
does, and it applies a **bounded modifier** to hero fit — at most ±5 points —
per `PRODUCT_SPEC.md` §19. The modifier makes no claim about what a hero
teaches: it only compares the player's own figures on that hero against their
own overall figures for the thing they are working on. It breaks ties between
heroes a player could reasonably pick; it never manufactures a recommendation.

One deliberate asymmetry: `GET /api/coach/training-focus` *selects* a focus
when none is set, and `GET /api/coach` does not. Reading the coach must not
have the side effect of committing a player to a goal.

---

## How the AI coach works

The rule the whole phase is built to enforce:

> The backend computes. The model interprets. An insight that cannot be traced
> back to a computed number does not get shown.

```text
metrics + benchmarks + hero pool
            │
            ▼
   evidence  (sentences this backend composed, each with a stable id)
            │
            ▼
     prompt  (JSON payload: ids, labels, statements, samples)
            │
            ▼
      model  (returns JSON: summary + insights citing evidence ids)
            │
            ▼
 validation  (unknown kind → dropped; unknown citation → stripped;
            │  no citation left → insight dropped; none left → 502)
            ▼
    storage  (coaching_analyses + coaching_insights)
```

### Evidence is the product, not the prose

Each piece of evidence is a sentence the backend wrote from its own figures:

```text
overall.record        Across 42 stored matches, you have won 24 and lost 18 (57%).
benchmark.gold_per_min  On Luna, your gold per minute averages 512; the peer median
                        is 500 and the top 20% start at 684. That places you at the
                        46th percentile.
match.deaths          That is 2.4 deaths per 10 minutes, against your average of 1.4.
```

The UI renders these verbatim, under the insight that cites them. **Every
number a user reads comes from this list**, so a model that hallucinates a
figure hallucinates it into a field nobody displays.

`GET /api/coach` returns the evidence whether or not a model is configured — it
is useful on its own, and a deployment with no `LLM_API_KEY` still has a
working coaching page.

### What the model may and may not do

The system prompt forbids arithmetic and requires a citation per insight, but
instructions are not a mechanism, so the answer is verified on the way back:

- **Kind** must be one of six (`strength`, `weakness`, `recurring_pattern`,
  `recommendation`, `warning`, `improvement`). An invented seventh is dropped,
  not mapped to the nearest real one.
- **Citations** are filtered against the evidence that was actually sent. An
  insight left with none is dropped; if nothing survives, the whole answer is
  discarded and the request answers `502` rather than showing unverified advice.
- **Lengths** are clamped on character boundaries, and the insight count is
  capped by `COACH_MAX_INSIGHTS`.
- Answers wrapped in prose or a ```` ```json ```` fence are still read; a
  truncated one (`finish_reason: "length"`) is rejected with a message that
  says so.

Nothing user-controlled reaches the model. The payload is built from typed
domain values and backend-composed sentences only — no persona name, no free
text, no provider blob — so there is no field for a player to write
instructions into.

What the validation guarantees is **provenance, not reasoning quality**: every
figure shown traces to a computed one, and every insight traces to evidence
that exists. It cannot catch a model that cites the right number and draws a
poor conclusion from it — that is a question of model choice, and the answer is
attributed with the model that produced it so a bad one is identifiable.

### Cost control

An LLM call is the only operation in this service that costs money per request,
so it is the only one with a budget:

- `POST` is the only verb that calls a model. Opening a page never does.
- Identical evidence is answered from storage. The cache key is a SHA-256 over
  the prompt version, scope, configured model and every evidence statement, so
  a repeated question costs nothing and does not consume the daily budget.
- `COACH_COOLDOWN_SECONDS` bounds frequency, `COACH_DAILY_LIMIT` bounds volume,
  and both are checked **before** the call rather than after it.

Generation is synchronous: one user-triggered call, bounded by
`LLM_TIMEOUT_SECONDS`. That needs no job queue. Background or scheduled
analysis would — and that is where a queue belongs when it arrives.

### Degradation

| What is down          | What still works                                    |
| --------------------- | --------------------------------------------------- |
| No `LLM_API_KEY`      | Everything except generation; `GET /api/coach` says so |
| Model unreachable     | All reads; `POST` answers `502` without leaking the cause |
| Benchmark provider    | Coaching, with fewer pieces of evidence             |
| Hero meta provider    | Coaching, with fewer pieces of evidence             |
| No payment provider   | Everything; checkout answers `503` and nobody is locked out |
| Payment provider down | Every read and every measured feature; only checkout fails |

---

## Billing: trial, subscription and settlement

Every account gets a **14-day trial** the moment it exists, and after that
**$9.98/month** buys the part of the product that costs money to run. The price,
the trial length and the period are configuration (`BILLING_PRICE_CENTS`,
`BILLING_TRIAL_DAYS`, `BILLING_PERIOD_DAYS`); they appear once, in
`BillingConfig`, and nowhere else in the system.

### What the subscription actually gates

Only the two endpoints that spend a model call: `POST /api/coach/analyze` and
`POST /api/matches/:id/analyze`. Stats, benchmarks, hero intelligence, the
player model, patterns, the training focus and every stored analysis keep
answering after the trial ends. An expired trial is a smaller product, not a
locked door — and the paywall is expressed once, by the `EntitledUser`
extractor, so a handler cannot forget it or implement it slightly differently.

### Entitlement is a function of timestamps

`Subscription::entitlement(now)` reads two windows and no status label: a paid
period that has not elapsed is `Pro`, a running trial is `Trial`, anything else
is `Free`. That is why `cancelled` and `past_due` behave correctly without
special cases — cancelling asks us not to renew, not to confiscate days already
bought — and why a row whose label is stale still grants exactly the right
thing. The label is corrected on the next read, and entitlement never waited
for it.

The trial itself is anchored to `users.created_at`, not to when the
subscription row happened to be materialised, so an account created before this
phase existed does not receive a fresh fortnight on its first visit to the
billing page.

### Settlement is signed, validated and idempotent

```text
POST /api/billing/checkout      reserve a payment row, then open the invoice
        ↓
provider hosted page            no card or wallet detail passes through us
        ↓
POST /api/billing/webhook       HMAC-SHA512 over the sorted JSON body
        ↓
record the event, then apply it in the same transaction
        ↓
subscription active until now + BILLING_PERIOD_DAYS
```

Four rules make that safe:

1. **The row exists before the provider is called.** The `order_id` we hand out
   is a payment row that already exists, so a notification can never arrive
   about a charge we have no record of.
2. **Nothing is believed before the signature.** The body is read as raw bytes
   and verified with a constant-time HMAC comparison; an unsigned or forged
   notification is never parsed for anything but the log line, and answers a
   `400` that explains nothing.
3. **Amount and currency are re-checked.** A signature proves who sent the
   message, not that it describes what we asked for. A charge settled for a
   different figure is refused outright — a cent does not buy a month.
4. **Every accepted event is recorded under a unique key in the transaction
   that applies it.** Providers retry; the unique index decides, so a
   redelivery is acknowledged with `"outcome": "duplicate"` and the paid window
   does not move.

Renewal extends from the current period end rather than from now, so paying
early never costs the days already bought. `POST /api/billing/checkout` reuses
an open charge instead of opening a second invoice, and refreshes it from the
provider first — which doubles as the recovery path for a notification that
never arrived, since the provider's own answer goes through exactly the same
application logic a webhook does.

### The provider boundary

`PaymentProvider` carries three verbs — open a charge, ask about a charge,
verify a notification — and no vendor vocabulary. NOWPayments' invoice API,
its status vocabulary and its `x-nowpayments-sig` header live behind it;
`partially_paid` maps to `confirming`, never to paid, and a status this build
has never heard of is an error rather than a guess. Swapping providers is a
`main.rs` change.

---

## Admin panel and vouchers

Three questions this surface exists to answer: how many people use the
product, what happens during their trial, and how many buy — plus a way to
grant access by hand, through a code, without a payment.

### Who gets in

`users.is_admin` is a boolean, set only by hand in the database — there is no
self-serve promotion path, and no separate roles table: two states (admin,
not-admin) do not earn a second place that has to agree with `is_admin`
forever. The `AdminUser` extractor wraps `CurrentUser`, so a **disabled**
admin is refused with `ACCOUNT_DISABLED` before the admin check ever runs —
disabling an account revokes the panel too, immediately, on its very next
request.

### The numbers are event-sourced, not derived from current state

`events` is one append-only table (`user_id, type, metadata, created_at`) that
every meaningful thing writes to: logins, logouts, `trial_started`,
`trial_expired`, `subscription_expired`, `purchase`, `voucher_redeemed`,
`feature_used`, `page_view`. `GET /api/admin/stats` is entirely `count`/`group
by` over this table plus `subscriptions` and `payments` — nothing is
estimated, and nothing here does arithmetic outside SQL.

Two metrics that look similar are computed differently on purpose:

- **DAU/WAU/MAU** count *distinct accounts* — ten logins from one person is
  one active user, not ten.
- **Daily purchases** count *raw events* — two charges from one account the
  same day are two purchases, unlike logins.

`trial_to_paid_conversion_pct` is a **cohort** figure: of the accounts whose
trial started inside `[from, to]`, what fraction have purchased by the time
the query runs — not "purchases inside the window," which would conflate two
different populations. Voucher redemptions are counted separately from that
conversion rate; redeeming a code is not a purchase.

### `subscriptions.source`: where access actually came from

Independent of `status` — a `trialing` row is always `trial`, but an `active`
one could have got there by paying, redeeming a voucher, or an admin granting
time directly. Whichever happened most recently overwrites it; there is one
current answer to "why does this account have access," not a history of every
way it ever got some. Money itself still lives only in `payments` — a
subscription can have many payments over its life, and vouchers/admin grants
have no amount at all, so nothing duplicates what `payments` already tracks
correctly.

### Vouchers: the concurrency guarantee is the database, not the pre-checks

```text
POST /api/admin/vouchers            DOTA-XXXX-XXXX, over an alphabet with
        ↓                           no 0/O or 1/I — a code read aloud or
count independent codes             retyped from a screenshot should never
                                     have an ambiguous character
        ↓
POST /api/subscribe/redeem          SELECT ... FOR UPDATE locks the voucher
        ↓                           row for the transaction
insert into voucher_redemptions     UNIQUE(voucher_id, user_id) — this is
        ↓                           what actually stops a double-redeem
        ↓                           under concurrency, not the check above it
extend the subscription             new_end = max(now, current_period_end)
                                     + duration_days — early redemption never
                                     costs days already granted
```

The checks that run first (voucher active, not expired, uses remaining, not
already redeemed by this account) exist for a clear error message, not the
guarantee — a second, simultaneous redemption attempt can pass every one of
them and still lose to the unique constraint. `POST /api/subscribe/redeem` is
rate limited at 5 attempts/minute per account, tracked in an in-memory
sliding window rather than a database write: a mistyped code is not a product
event worth a row, and this keeps abuse-prevention noise out of `events`
entirely.

### Audit log

Every mutating admin route — extend, disable, enable, create a voucher,
deactivate one — writes one row to `admin_audit_log`
(`admin_id, action, target_type, target_id, metadata, created_at`) before
answering, readable at `GET /api/admin/audit-log`. Best-effort, like the
`events` table: a logging failure is warned about and never blocks the
action itself, because losing one audit entry is a smaller problem than an
admin who can't disable a malicious account because logging it failed.
`admin_id` is nullable and `ON DELETE SET NULL` — the admin's own account
being deleted later must not erase the historical record that they took the
action.

---

## How the AI coaching pipeline will work

Phases 7-9 build on the metrics, benchmark and hero layers. The shape is fixed even
where the code is not yet written:

1. **Normalize.** The provider converts payloads into domain models. Duplicates
   are impossible: the planner diffs stored ids and `UNIQUE (dota_player_id,
   match_id)` backstops concurrent syncs.
2. **Compute.** Deterministic metrics, above.
3. **Benchmark.** Done — see above. Currently hero-scoped; rank and role
   segmentation wait on a provider that offers them.
4. **Aggregate.** Hero pool done — see above. The player model and recurring
   patterns evolve as matches arrive rather than being rebuilt per match.
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

### The competitive match population

Everything the coach asserts — overall statistics, role performance, the role
recommendation, the coaching dataset, the evidence a model is shown — is a
statement about **standard All Pick matchmaking**. Turbo is a different game
with a different economy curve, so mixing it in does not enlarge the sample, it
corrupts it.

One rule decides it, in `domain::eligibility`, and every caller reads it from
there. A match is eligible when:

```text
game_mode  ∈ { 22 all_draft, 1 all_pick }
lobby_type ∈ { 7 ranked, 0 normal }
```

Both dimensions are required. `lobby_type` alone would admit Turbo, which
normally sits in a normal lobby; `game_mode` alone would admit All Pick played
in a tournament, a practice lobby or Battle Cup. The ids are verified against
OpenDota's own `/api/constants/game_mode` and `/api/constants/lobby_type`. A
match whose mode the provider never reported is excluded rather than assumed —
a guess there would quietly put Turbo back into the numbers.

The rule exists in two renderings, Rust and SQL, both generated from one pair of
constant lists with a test asserting they cannot drift apart.

Two things follow that are easy to get wrong:

- **Syncing still stores everything.** The filter is a read rule. A player's
  match list shows the Turbo games they actually played; the coaching numbers
  do not read them. Exclusions are counted and reported by reason, never
  silently dropped.
- **The window is taken after filtering, never before.** "The latest 100
  matches, minus Turbo" gives a Turbo-heavy player a fifty-game analysis that
  claims to be a hundred-game one. `domain::scope::MatchScope` renders
  filter → order by recency → limit as one SQL window, so the ordering is
  structural rather than a convention.

Note that the sync limit counts *raw* matches, so filling a hundred-match
competitive window can require pulling considerably more than a hundred —
see `SYNC_MATCH_LIMIT`.

### Role performance, and why it is scored this way

The five coachable roles are Carry, Mid, Offlane, Soft Support and Hard
Support. The estimator's other labels — `Core`, `Roamer`, `Jungle`, `Unknown` —
map to none of them and are counted as **unclassified**. `Core` is the costly
one: it means farm priority identified a core without identifying which lane,
and folding it into Carry would put Mid and Offlane games into a Carry player's
coaching dataset. A player with mostly unparsed replays will therefore see a
large unclassified count and small per-role samples. The alternative is
coaching someone's Carry on their Offlane games.

Each role's 0-100 score combines four **role-neutral** measures:

| Measure               | Default weight | 0 means        | 100 means       |
|-----------------------|---------------:|----------------|-----------------|
| Win rate              |           0.45 | never wins     | always wins     |
| Kill participation    |           0.20 | never involved | every team kill |
| KDA                   |           0.20 | 0.0            | 6.0 or better   |
| Deaths per 10 minutes |           0.15 | 3.0 or worse   | none            |

Gold per minute is deliberately absent. Comparing a hard support's economy with
a carry's would recommend the safe lane to everyone, which is advice about Dota
rather than about the player — so economy is reported beside the score and not
inside it. A measure with no data (kill participation needs team totals the
provider does not always supply) is dropped and the remaining weights
renormalized, never counted as zero.

The result is then pulled toward the midpoint by how thin the evidence is:

```text
performance = 50 + (raw - 50) × n / (n + 10)
```

and no role with fewer than ten eligible matches is recommended at all. Both
are needed: shrinkage narrows the gap a hot streak opens, the floor is what
actually stops five perfect games outranking forty consistent ones. A role under
the floor is still shown with its real numbers — the recommendation is advice,
and the player picks the role they want to improve.

Sample confidence is reported alongside every analysis: `Limited` below 20
eligible matches, `Moderate` below 60, `Strong` at or above it.

### What the Top 20% comparison can and cannot say

The spec asks to benchmark a player against the top 20% of **the same rank,
role and hero**. One of those three is deliverable from the current provider,
and the response says which.

OpenDota's `/benchmarks` returns one percentile distribution per hero:
`gold_per_min`, `xp_per_min`, `last_hits_per_min` and the rest, at p10 through
p99 — so the 80th percentile is a real "top 20% starts here" line, and the gap
to it is arithmetic. What it does not return is any segmentation beyond the
hero. That is verified rather than assumed: passing `rank`, `rank_tier` or
`lane_role` to the endpoint returns byte-identical buckets, and the API
publishes no sample size, no patch and no statement of which game modes the
distribution covers.

So the two halves of the comparison are described separately on every response:

- **Our half is genuinely narrow.** The player's own averages come from the
  eligible Ranked and public All Pick matches in the selected role, on that
  role's most-played hero. A safe-lane Phantom Assassin average never reaches a
  support comparison.
- **The peer half is wide and undocumented**, and `context.population.peers`
  says so in those words.

`context.requested` lists the four dimensions the product asks for,
`context.segmented_by` the one that arrived, and `context.unavailable` names
each missing dimension with the reason. `context.population.comparable` is
`false` — an unknown population cannot be declared equal to a known one, and
flipping it to `true` requires a source that publishes what it covers (STRATZ
is the candidate).

The caveats are built on every path, including the provider-outage path. A
response whose honesty notes vanish exactly when the data gets thinner would
have them backwards.

### The coaching profile

Overall analysis and role coaching are different questions, and the profile is
the line between them. It stores one thing that cannot be derived — **which
role the player chose to work on** — plus the circumstances of that choice:
what was recommended at the time, and how many eligible matches the advice
rested on. Neither can be reconstructed later, because the analysis moves as
matches arrive.

It deliberately does **not** store the match ids the coaching dataset resolves
to. Those change on every sync, so persisting them would create a second answer
to "which matches is this advice about", and the stale one would win whenever
somebody read it. The dataset is resolved from the role on every read.

### Where the scope is visible

Two populations exist, so every screen says which one it is reading — in
words, not by implication.

- **Overview** leads with the analysed count, the sample confidence and an
  expandable breakdown of what was excluded and why.
- **Matches** defaults to the player's whole history, because their Turbo games
  are theirs to look at, and labels every row with the mode it was played in
  plus a "not coached" mark where the coach does not read it. A tab switches
  the list to the competitive population, which is the same query the dashboard
  uses. This is what reconciles "seventeen matches" in the list with "ten
  matches analysed" on the dashboard; without it the two screens look like they
  disagree.
- **Coach** states the role, its games, its score and the window above
  everything, keeps "Change role" one tap away, and says plainly when the
  player chose against the advice.
- **Benchmark** states both populations and every dimension it could not
  segment on.

The mode label and the eligibility flag are computed server-side and rendered
verbatim. Shipping the mode tables to the browser would create a second copy of
the rule, and the first thing a second copy does is drift.

### Why a role was recommended

The recommendation ships with its reasoning, because it is advisory and a
player overriding it deserves to see what they are overriding: each measure,
the value it was measured at, and the share of the score it carries — including
the fact that weights are renormalised per role around any measure that role
had no data for. Where the sample-size adjustment moved a score, both numbers
are shown ("measured 85/100 across 5 games, reported as 62"), and a role below
the recommendation floor says how many more games it needs rather than being
silently absent from the advice. It stays selectable throughout: a thin sample
is a reason not to recommend a role, never a reason to refuse it.

### What the model may say

The coaching layer treats the model as an interpreter that is not trusted on
the way back, and it checks two different things.

**Citations.** An insight or plan step must name at least one evidence id it
was actually shown. Ids that do not exist are stripped; anything left with no
citation is dropped.

**Figures.** A citation says where a claim came from; it does not say the claim
is true to it. A model can cite `overall.deaths` and then write "you die 7.4
times per 10 minutes" when the evidence says 4.1 — a real id attached to an
invented number, which reads exactly like a verified one. So every figure in an
insight, plan step or summary is matched against the evidence behind it.
Rounding down in precision is accepted (quoting 4.1 for 4.14, or 512 for
511.8), because that is how anyone writes a measured number into a sentence.
Inventing precision, or a number that is not there at all, is not: the insight
or step is dropped, and the summary is discarded while the verified insights
around it survive.

That strictness is only fair because the prompt is explicit about it. The model
is told that figures are checked after it answers, and that advice should be
qualitative — "push your first item earlier" rather than "before 18 minutes",
which is a timing nobody measured and which the checker will throw away.

The cost is real and worth stating: a genuinely useful piece of advice phrased
with a concrete target is lost along with the fabrications. The alternative is a
coach that is occasionally, confidently wrong about a number the player will act
on, which is the worse failure for a product whose entire claim is that the
numbers are computed rather than generated.

### How the role scope is enforced

Once a role is chosen, every coaching read is built from one `MatchScope` —
eligible matches, that role, newest first, capped at the window — and that
scope is threaded through every query behind the answer: the aggregate
statistics, the recent-match list, the hero pool and its baseline, the player's
own benchmark averages, and the pattern detectors. A support match cannot reach
a carry analysis because it was never fetched, not because a prompt asked a
model to ignore it.

Three consequences worth stating, because each was a bug waiting to happen:

- **The role is part of the cache key.** Analyses are keyed by a hash of the
  evidence, and two roles can produce byte-identical evidence. Without the role
  in the hash — and in the `latest()` filter — a player who switched roles
  would be served their previous role's analysis, silently, looking exactly
  like a fresh one.
- **The training focus is per role.** "One thing to work on" is one thing *per
  role*; switching to carry for a week must not retire the support goal waiting
  behind it.
- **Patterns are detected inside the scope but not persisted there.**
  `player_patterns` is keyed by player and pattern id, so a carry pattern and a
  support pattern would overwrite each other and `first_detected_at` would mean
  neither. Role-scoped patterns are therefore computed fresh on each read and
  have no first-sighting date; the persisted ones are competitive-wide across
  every role.

Two things are deliberately **not** role-scoped. The long-term player model
answers "which roles do you actually play", which narrowing would make
circular; and the hero pages describe the player's whole repertoire. Both are
labelled in the UI where they sit beside role-scoped panels.

Coaching refuses to answer at all before a role is chosen — `409`, with a
message that distinguishes "you have not chosen yet" from "you have nothing
synced yet". Any default would either mix roles or quietly overrule the
player's decision.

The recommendation is recorded beside the selection precisely so the two can
disagree. A player advised to work on Soft Support who picks Carry is coached
on Carry; the profile remembers both, and the UI says so. Any of the five roles
can be chosen, including one with no matches behind it — wanting to learn a
position you do not play is a normal reason to open a coaching app, and the
empty dataset is a fact for the coaching screen to report rather than a reason
to refuse the choice.

---

## Installable app, offline behaviour and observability

### The PWA

The frontend installs: a manifest route (`/manifest.webmanifest`), maskable and
plain icons rendered from one SVG, `display: standalone`, and a theme colour
that matches the shell so an install does not flash white on launch. On a phone
the installed app is the design the UI was built for in the first place — the
bottom tab bar is the navigation once there is no URL bar above it.

The service worker (`public/sw.js`) is deliberately small, and deliberately not
a data cache:

- **`/api/` is never intercepted.** Every response is session-scoped and
  time-sensitive; a cached benchmark or, worse, a cached entitlement is a wrong
  answer that looks like a right one.
- **Only content-hashed assets are cached.** `/_next/static/` URLs change when
  their bytes change, so a hit is always current.
- **Navigations fall back rather than go stale.** With no network, `/offline`
  explains that the connection is missing and the data is not — it makes no
  claims about the account, because it cannot check any.

`sw.js` itself is served `no-store`, so a deploy cannot be shadowed by a worker
the browser cached yesterday. In development the worker is unregistered instead
of installed: a cached shell in front of a hot-reloading dev server is exactly
the bug whose fix is "hard refresh".

### Error and loading states

Failures are handled where they happen. A component whose request fails renders
a message in place, because losing a whole page over one unavailable panel is a
worse answer — that is why an LLM outage still shows the evidence, and a lapsed
trial still shows every measured number. Above that sit three boundaries for
the cases a component cannot absorb: `error.tsx` for a render that threw (retry
plus the digest to quote), `global-error.tsx` for a failure in the root layout
itself (fully inline, since no CSS or font is guaranteed), and `not-found.tsx`
for an address that does not exist.

Loading is the same story twice: a route-level `loading.tsx` while the route's
code is in flight, then each component's own skeleton while its data is. Both
use the same pulsing panels, so the handover is not a second visible jump.

### Observability

Every request gets an id — the caller's `x-request-id` if it sent a usable one,
a fresh UUID otherwise — attached to a span around the whole request, echoed
back in the response, and exposed through CORS so a user can quote it. It
appears on every log line the request produced, including ones emitted three
layers down, which is what makes "it failed at 14:05" findable.

An inbound id is treated as untrusted input: it ends up in log lines, so
anything that is not a short plain token is replaced rather than repeated. A
newline in a header must never be able to forge a log entry.

One line closes each request with its method, path, status and latency, at
`error` for a `5xx` and `info` otherwise. `/health` reports the database and
whether an LLM is configured; `/health/live` never touches a dependency, so a
liveness probe cannot be taken down by Postgres.

The backend also says what is wrong with a deployment at boot rather than
leaving it to be discovered from a failing login: `COOKIE_SECURE` disagreeing
with the scheme in either direction, an empty `CORS_ORIGINS`, or a
`FRONTEND_BASE_URL` that is not in it.

### Production checklist

| Setting                              | Why                                                     |
| ------------------------------------ | ------------------------------------------------------- |
| `COOKIE_SECURE=true`                 | Sessions over HTTPS only. False outside localhost is a warning at boot. |
| `COOKIE_CROSS_SITE=true` *only if the frontend is on another site* | `SameSite=Lax` is never returned cross-site, so the session silently never arrives. Refused unless `COOKIE_SECURE=true`. |
| `PUBLIC_BASE_URL` on HTTPS           | Steam signs `return_to`; it must match what the browser reaches. |
| `CORS_ORIGINS` = the real frontend origin | Credentials are allowed, so a wildcard is impossible.  |
| `NEXT_PUBLIC_API_URL` = public API URL | Baked into the bundle *and* into the CSP `connect-src`; rebuild the image to change it. |
| `NOWPAYMENTS_*` set, or `BILLING_ENFORCE` left alone | Otherwise checkout answers `503` — and, by default, nobody is locked out either. |
| `LLM_API_KEY` set                    | Without it the product runs; only generation is unavailable. |
| `RUST_LOG`                           | `dota_coach_backend=info` keeps one line per request.   |

The frontend ships strict response headers: a CSP with no external origins
beyond Valve's image CDNs and the API, `frame-ancestors 'none'`, `nosniff`,
`strict-origin-when-cross-origin` and a `Permissions-Policy` that turns off
camera, microphone, geolocation and payment. `'unsafe-eval'` and websockets are
added **only** in development, where React Refresh needs them.

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
  hashing, cookie flags, OpenID claimed-id parsing, pagination validation,
  entitlement windows and period arithmetic, IPN signature verification
  (including a reordered body and a tampered amount), request-id sanitization
  and error mapping. No database, no network.
- **Integration tests** (`backend/tests/api.rs`) drive the *real* router with a
  mock `DotaDataProvider` and a stub `SteamVerifier`, against a real Postgres.
  They cover the login round trip, rejection of anonymous, forged and expired
  sessions, login-nonce mismatches, sync idempotency, duplicate match ids,
  provider outages/rate limits/bad responses, pagination, metric computation
  and invalidation, benchmark ranking and its refusal to rank a thin sample,
  and that one user can read neither another's matches nor another's
  statistics. Billing is covered end to end: the trial a new account gets
  without asking, the fact that materialising it late hands out no extra days,
  the paywall closing generation while every measured endpoint keeps answering,
  checkout reusing an open charge instead of opening a second invoice, an
  unsigned or forged notification buying nothing, a verified one activating the
  subscription, a redelivery not buying a second month, a wrong amount granting
  nothing, and a settled charge that a later notification cannot reopen. The
  public plan endpoint is covered too: readable without a session, carrying
  pricing copy and nothing else, and every response — including a `401` —
  carrying a request id, with a hostile one replaced rather than echoed.

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
- **Entitlement is never taken from the client.** Trial and subscription state
  are derived from stored timestamps on every request, and a subscription is
  activated only by a notification the provider signed — never because a
  browser returned from a checkout page saying it went well.
- **The payment webhook trusts only its HMAC.** It is unauthenticated by
  necessity, verified in constant time over the exact bytes received, validated
  against the charge it names, and idempotent by a unique index.

---

## Known limitations

- **One Dota account per user.** Enforced by `UNIQUE (user_id)` on
  `dota_players`; dropping that constraint is all a multi-account feature needs.
- **Steam profile comes from OpenDota**, not the Steam Web API, so a brand-new
  account shows no persona until the first sync. Avoiding a second provider and
  a second API key was the tradeoff.
- **`role` is an estimate.** See [Why roles are estimates](#why-roles-are-estimates).
- **Sync and coaching are synchronous.** Both hold the request open for a few
  seconds; there is no job queue. That is fine while every call is
  user-triggered and bounded by a timeout — scheduled or background analysis is
  what would need a queue.
- **Model quality is a deployment choice.** The prompt carries a worked example
  because small models otherwise echo the evidence back instead of interpreting
  it; the validator catches that (`502`, nothing stored), but the feature is
  only as good as the model behind `LLM_MODEL`.
- **The coach only knows what the evidence says.** It cannot see drafts, item
  builds, positioning or comms, so its advice is bounded by what the metrics,
  benchmark, hero and pattern layers measure.
- **Most pattern detectors need a parsed replay.** Laning, item timings and
  the won-lane check only run on matches OpenDota parsed, which is a minority
  of public games. Those detectors stay silent and say so, with the count they
  managed, rather than reporting a clean laning phase nobody measured.
- **Only two benchmark metrics can become a training focus.** A metric
  qualifies only when the same quantity can be read back out of a single
  stored match, because otherwise there is no honest way to plot progress
  against it — so gold per minute and deaths qualify, and the rest reach the
  coach as evidence but never as a goal.
- **Pattern thresholds are documented constants, not a skill rating.** Three
  deaths per 10 minutes is the line between "worth mentioning" and "not worth
  mentioning". Comparison against real players is the benchmark engine's job.
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
- **Subscriptions do not auto-renew.** A period is bought by one settled
  charge; when it lapses the account returns to the measured product and has to
  check out again. Recurring crypto billing is a provider feature this does not
  use.
- **An expired window is corrected lazily.** There is no sweep job: a stale
  `trialing` or `active` label is rewritten the next time that account is read.
  Entitlement is computed from the timestamps either way, so the label is
  bookkeeping.
- **A lost notification needs a visit.** If the provider's callback never
  arrives, the charge is reconciled the next time checkout is opened, not by a
  background poller.
- **Offline means offline.** The service worker caches the app shell, not data:
  every number in this product is computed by the backend, so without a
  connection the app says so rather than showing yesterday's figures. There is
  no background sync and no push.
- **Observability stops at logs.** Request ids, one line per request and the
  health endpoints are all there is — no metrics endpoint, no tracing exporter,
  no error aggregator.
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
| 6     | Hero intelligence: meta providers, hero pool, fit score        | ✅ done |
| 7     | AI coach: LLM provider, evidence-based insights                | ✅ done |
| 8     | Player model and recurring pattern detection                   | ✅ done |
| 9     | Training focus and progress tracking                           | ✅ done |
| 10    | Trial, entitlements, crypto billing, webhooks                  | ✅ done |
| 11    | PWA, landing page, production configuration, observability     | ✅ done |

Deliberately **out of scope** until the core loop is excellent: microservices,
live overlay, voice coaching, replay parsing, native apps, social features,
leaderboards, full draft assistant.
