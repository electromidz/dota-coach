# Dota Coach — Claude Code Instructions

## 1. Project Context

Dota Coach is a production-oriented SaaS **Personal AI Coach for Dota 2**.

The product is designed to:

- Analyze a player's Dota 2 match history
- Calculate deterministic performance metrics
- Benchmark the player against appropriate peers
- Understand the player's personal hero pool
- Combine current Dota meta with personal performance
- Detect recurring weaknesses and strengths
- Generate personalized coaching insights
- Select one primary training focus
- Track improvement over time

This is NOT intended to be just a generic AI match analyzer.

The long-term product loop is:

```text
Steam
  ↓
Dota Player
  ↓
Match History
  ↓
Deterministic Metrics
  ↓
Benchmark
  ↓
Hero Intelligence
  ↓
Player Model
  ↓
Recurring Patterns
  ↓
AI Coach
  ↓
Training Focus
  ↓
Progress Tracking
```

---

# 2. Source of Truth

Before making architectural or product decisions, read:

```text
PRODUCT_SPEC.md
```

`PRODUCT_SPEC.md` is the primary product and architecture specification.

When the existing implementation conflicts with the specification:

1. Inspect the existing implementation.
2. Determine whether the specification or existing code is more current.
3. Do not blindly rewrite existing code.
4. Explain the conflict before making a destructive architectural change.

The repository's existing working code is valuable and should be preserved whenever possible.

---

# 3. General Development Rules

Before modifying code:

1. Inspect the relevant files.
2. Understand the current implementation.
3. Search for existing abstractions before creating new ones.
4. Reuse existing code where appropriate.
5. Make the smallest safe change.
6. Keep domain logic independent from external providers.
7. Add or update tests for important behavior.
8. Run the relevant tests/build.
9. Review the resulting diff.
10. Report what changed and why.

Do NOT rewrite large parts of the application just to make the code match a preferred architecture.

Prefer incremental refactoring.

---

# 4. Architecture

The backend should remain a **modular monolith**.

Do NOT introduce microservices unless explicitly requested.

Preferred conceptual modules:

```text
auth/
users/
dota/
metrics/
benchmarks/
heroes/
player_model/
patterns/
coaching/
billing/
repositories/
database/
api/
```

The exact directory structure may differ from the specification if the existing project already has a better equivalent structure.

The important rule is separation of responsibilities.

---

# 5. Provider Abstraction

External services must be isolated behind interfaces/traits.

Important provider boundaries include:

```rust
trait DotaProvider {
    ...
}

trait HeroMetaProvider {
    ...
}

trait BenchmarkProvider {
    ...
}

trait PaymentProvider {
    ...
}

trait LlmProvider {
    ...
}
```

Domain logic must not depend directly on:

```text
OpenDota
STRATZ
Dotabuff
NOWPayments
OpenAI
```

Provider-specific implementations belong behind the appropriate abstraction.

---

# 6. Dota Data Providers

OpenDota is the initial Dota data provider.

The architecture must allow STRATZ or another provider to be added later without rewriting the domain layer.

Do not spread OpenDota-specific response types throughout the application.

Normalize external data into internal domain models.

---

# 7. Hero Meta Providers

Hero meta is part of **Hero Intelligence**.

Preferred provider strategy:

```text
STRATZ
   ↓
OpenDota fallback
   ↓
Dotabuff optional
```

Do NOT make Dotabuff a hard dependency.

Do NOT build the core application around scraping Dotabuff HTML.

If Dotabuff is implemented, isolate it behind:

```rust
struct DotabuffHeroMetaProvider;
```

The rest of the application must not know whether meta data came from STRATZ, OpenDota, or Dotabuff.

Before implementing or changing an external provider:

- Verify that the API/data source is currently available.
- Verify authentication requirements.
- Verify rate limits.
- Verify response structure.
- Verify that the intended usage is appropriate.
- Never invent fields that the provider does not actually expose.

If an external provider is unavailable, keep the provider boundary intact rather than creating fake production data.

---

# 8. Deterministic Metrics

The backend is the source of truth for all numerical calculations.

The LLM must NOT calculate:

- GPM
- XPM
- KDA
- Percentiles
- Benchmark values
- Win rates
- Sample sizes
- Match statistics
- Progress values

These must be calculated deterministically in Rust.

The LLM interprets structured results.

---

# 9. Benchmarking

Benchmarking is a first-class domain feature.

Benchmark context may include:

```text
Hero
Role
Rank / MMR bracket
Patch
Metric
```

Benchmark calculations must be deterministic.

Do not make percentile claims when the sample size is insufficient.

Prefer:

```text
Insufficient sample size
```

over fabricated precision.

Benchmark results should be explainable.

---

# 10. Hero Intelligence

Hero Intelligence is a core product feature.

The system should answer:

> Among the heroes that are currently strong, which ones are actually a good fit for this player?

Hero recommendations should consider:

```text
Current Meta
+
Player Hero Pool
+
Player Performance
+
Player Experience
+
Benchmark Performance
+
Recent Form
+
Training Focus
```

Do NOT simply recommend the hero with the highest global win rate.

A strong meta hero may still be a bad recommendation if:

- The player has little experience on it.
- The player performs poorly on it.
- Recent performance is poor.
- It does not fit the player's role.
- It conflicts with the current coaching objective.

Likewise, a slightly weaker meta hero can be an excellent recommendation if it is already one of the player's strongest heroes.

---

# 11. Hero Fit Score

Hero Fit Score must be deterministic and explainable.

It may combine:

```text
Player Performance
Meta Strength
Player Experience
Benchmark Performance
Recent Form
Training Focus Compatibility
```

Initial weights are defined in `PRODUCT_SPEC.md`.

Do not hard-code the scoring architecture so tightly that the weights cannot evolve.

The scoring system should be easy to tune later.

---

# 12. Player Model

The application should build a persistent understanding of the player.

The Player Model should eventually contain:

```text
Strengths
Weaknesses
Hero Pool
Preferred Roles
Recurring Patterns
Benchmark Profile
Recent Form
Training History
Confidence
```

Do not treat every match as an isolated analysis.

Historical context matters.

---

# 13. Recurring Patterns

Do not identify a recurring weakness from a single match.

A recurring pattern requires sufficient evidence across multiple matches.

Examples:

```text
Repeated unnecessary deaths
Poor LH@10
Late item timings
Strong laning but poor mid-game conversion
Low teamfight participation
Poor objective participation
Repeatedly throwing an early advantage
```

Whenever possible, connect a pattern to measurable evidence.

---

# 14. AI Coach

The LLM should receive structured domain information.

Typical input:

```text
Player Profile
Recent Matches
Metrics
Benchmarks
Hero Pool
Hero Intelligence
Recurring Patterns
Recent Form
Training Focus
```

The LLM should explain:

```text
What happened?
Why does it matter?
What pattern is emerging?
What should the player change?
```

The LLM is an interpretation layer, NOT the source of truth.

---

# 15. Training Focus

The coaching system should generally select **one primary training focus**.

Avoid overwhelming users with a long list of weaknesses.

Training Focus should consider:

```text
Benchmark Gap
Historical Pattern
Recent Performance
Impact
Confidence
Recency
```

Do not simply select the lowest numerical statistic.

The goal is meaningful player improvement.

---

# 16. Authentication

Steam authentication must be implemented through Steam OpenID.

Important rules:

- SteamID must be derived server-side.
- Never trust a SteamID supplied by the frontend.
- Never request Steam passwords.
- Sessions must be server-side.
- Session identifiers must not be exposed to JavaScript.
- Use HTTP-only cookies.
- Use Secure cookies in production.
- Use appropriate SameSite protection.

---

# 17. SaaS Entitlements

Trial and subscription state are backend responsibilities.

The frontend must never be the source of truth for:

```text
Trial active
Subscription active
Payment successful
Premium access
```

Centralize entitlement checks.

The initial trial is:

```text
14 days
```

The initial subscription price is:

```text
$1/month
```

But the price must remain configurable.

---

# 18. Payments

Payments must be abstracted:

```rust
trait PaymentProvider {
    async fn create_payment(...);
    async fn get_payment_status(...);
    async fn handle_webhook(...);
}
```

Initial provider may be NOWPayments.

Do not tightly couple business logic to NOWPayments.

Payment webhooks must be:

- Verified
- Validated
- Idempotent
- Server-controlled

Never activate a subscription solely because the frontend says payment succeeded.

---

# 19. Security

Always consider:

- Authentication
- Authorization
- IDOR
- Session security
- Cookie security
- Input validation
- SQL injection
- Secrets management
- Webhook verification
- Provider API key protection
- Rate limiting where appropriate

Never expose provider API keys to the frontend.

Never commit secrets.

---

# 20. Database

Use PostgreSQL through SQLx.

Database schema should reflect domain boundaries.

Important entities include:

```text
users
sessions
dota_players
matches
match_metrics
benchmark_results
hero_pool
hero_meta_snapshots
hero_recommendations
player_patterns
training_focus
subscriptions
payments
```

Do not create tables simply because they are listed here if the existing implementation already provides an equivalent design.

Prefer normalized, queryable domain data over storing everything as opaque JSON.

---

# 21. API Design

Follow REST-style API conventions.

Important routes include:

```text
/api/auth/*
/api/players/*
/api/matches/*
/api/stats
/api/benchmark/*
/api/heroes/*
/api/hero-intelligence
/api/coach/*
/api/billing/*
```

Keep authentication and entitlement middleware centralized.

Do not duplicate access-control logic inside every handler.

---

# 22. Frontend

The frontend should be feature-oriented.

Conceptual areas:

```text
auth
dashboard
matches
benchmark
heroes
hero-intelligence
coach
billing
account
```

The dashboard should answer:

> What should I work on right now?

not merely display statistics.

Prioritize:

```text
Training Focus
↓
Why it matters
↓
Progress
↓
Hero Recommendation
↓
Recent Matches
↓
Benchmark
```

---

# 23. Error Handling

Never hide provider failures.

Use meaningful errors such as:

```text
ProviderUnavailable
RateLimited
InvalidResponse
AuthenticationFailed
NotFound
InsufficientData
```

External provider failure should not unnecessarily bring down the entire application.

For example:

```text
Hero meta unavailable
```

should not prevent the user from viewing their match history and existing coaching information.

---

# 24. Caching

Avoid unnecessary calls to external providers.

Cache or persist appropriate data.

Examples:

```text
Match data
Hero meta
Benchmark data
Player synchronization
```

Respect provider rate
