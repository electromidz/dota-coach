# Dota Coach — Master Product & Engineering Specification

You are working on an existing project called **Dota Coach**.

Repository:
`https://github.com/electromidz/dota-coach`

Your job is to evolve the existing application into a production-oriented **SaaS Personal AI Coach for Dota 2**.

Do NOT blindly rewrite the project.

First inspect the existing repository, understand its current architecture and implementation, and then implement the specification below incrementally while preserving working code.

---

# 1. Product Vision

Dota Coach is not simply a match analyzer.

It should behave like a **long-term personal Dota 2 coach**.

The core questions the product should answer are:

- What am I good at?
- What am I bad at?
- What mistakes keep repeating?
- Which heroes fit me?
- Which heroes are currently strong for my rank and role?
- Which of those meta heroes fit my personal playstyle?
- What should I work on right now?
- Is my performance improving?
- Am I getting closer to the benchmark for better players?

The core product loop is:

```text
Steam Login
    ↓
Dota Player Identification
    ↓
Match History
    ↓
Deterministic Metrics
    ↓
Hero Pool Analysis
    ↓
Peer Benchmarking
    ↓
Hero Intelligence
    ↓
Player Model
    ↓
Recurring Pattern Detection
    ↓
AI Coaching
    ↓
Training Focus
    ↓
Future Matches
    ↓
Progress Tracking
    ↓
Updated Player Model
```

The system should continuously learn from the player's history.

---

# 2. Important Engineering Principles

## 2.1 Modular Monolith

Keep the application as a **modular monolith**.

Do NOT introduce microservices unless there is a very strong technical reason.

Backend:

- Rust
- Axum
- Tokio
- Serde
- SQLx
- PostgreSQL

Frontend:

- Next.js
- TypeScript
- Tailwind CSS
- PWA architecture

---

## 2.2 Provider Abstraction

External data providers must never leak deeply into the domain layer.

Use interfaces/traits such as:

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

The domain and coaching engine should depend on abstractions, not directly on OpenDota, STRATZ, Dotabuff, or a specific payment provider.

---

# 3. Authentication

Authentication is part of the MVP.

Users must authenticate through **Steam**.

Use Steam OpenID.

Do NOT ask the user to manually enter a Steam ID during normal onboarding.

Flow:

```text
User
 ↓
Login with Steam
 ↓
Steam OpenID
 ↓
Backend validates identity
 ↓
Backend derives SteamID
 ↓
Find/Create internal User
 ↓
Find/Create Dota Player
 ↓
Create server-side session
 ↓
HTTP-only secure cookie
```

Required endpoints:

```text
GET  /api/auth/steam
GET  /api/auth/steam/callback
GET  /api/auth/me
POST /api/auth/logout
```

Security requirements:

- HTTP-only cookies
- Secure cookies in production
- SameSite protection
- Server-side sessions
- Session expiration
- Session revocation
- Never trust SteamID supplied by frontend
- Never store Steam passwords
- Never expose session IDs to JavaScript

---

# 4. User Model

Create/maintain:

```text
users
```

Fields:

```text
id
steam_id
display_name
avatar_url
created_at
updated_at
last_login_at
```

Constraints:

- `steam_id` must be unique
- Internal user ID must be used throughout the application
- Steam identity is the source of truth for authentication

---

# 5. Sessions

Create:

```text
sessions
```

Fields:

```text
id
user_id
expires_at
created_at
last_seen_at
```

Session middleware should resolve the authenticated user.

Do not duplicate authentication logic throughout individual endpoints.

---

# 6. Dota Player Identity

Create:

```text
dota_players
```

Fields:

```text
id
user_id
steam_id
dota_account_id
created_at
updated_at
```

Normally:

```text
SteamID → Dota Account ID
```

must be resolved automatically.

Do not require manual account ID input during normal onboarding.

---

# 7. SaaS Trial

The application must support a **14-day free trial**.

Trial is a backend entitlement.

Never trust the frontend to determine whether a user is still in trial.

Example:

```text
NEW USER
   ↓
14 DAY TRIAL
   ↓
TRIAL ACTIVE
   ↓
TRIAL EXPIRED
   ↓
PAYMENT REQUIRED
   ↓
PRO
```

Create:

```text
subscriptions
```

Fields:

```text
id
user_id
status
plan
trial_started_at
trial_ends_at
current_period_start
current_period_end
provider
provider_customer_id
provider_subscription_id
created_at
updated_at
```

Possible statuses:

```text
trialing
active
expired
cancelled
past_due
```

Centralize entitlement checks.

Example:

```rust
enum Entitlement {
    Free,
    Trial,
    Pro,
}
```

The backend must decide whether a user can access premium features.

---

# 8. Crypto Billing

Initial subscription price:

```text
$9.98/month
```

But do NOT hard-code this throughout the application.

The price must be configurable.

Use a payment provider abstraction:

```rust
trait PaymentProvider {
    async fn create_payment(...);
    async fn get_payment_status(...);
    async fn handle_webhook(...);
}
```

Initial implementation can use a crypto payment provider such as **NOWPayments**.

Do not couple the domain layer directly to NOWPayments.

Create:

```text
payments
```

Fields:

```text
id
user_id
subscription_id
provider
provider_payment_id
amount
currency
status
payment_url
created_at
updated_at
completed_at
```

Required endpoint:

```text
POST /api/billing/checkout
POST /api/billing/webhook
GET  /api/billing
GET  /api/billing/subscription
GET  /api/billing/payments
```

Webhook requirements:

- Verify authenticity/signature according to provider
- Validate payment/order ID
- Validate amount
- Validate currency
- Validate status
- Idempotent processing
- Never activate subscription based solely on frontend confirmation
- Backend is the source of truth

---

# 9. Dota Data Provider

The system should retrieve Dota match/player information through a provider abstraction.

Example:

```rust
trait DotaProvider {
    async fn get_player(...);
    async fn get_matches(...);
    async fn get_match(...);
}
```

Initial provider:

```text
OpenDota
```

The architecture must allow a future STRATZ implementation without rewriting the domain layer.

---

# 10. Match Storage

Store relevant match information locally.

Example:

```text
matches
```

Possible fields:

```text
id
dota_match_id
player_id
hero_id
hero_name
role
result
duration
kills
deaths
assists
gpm
xpm
last_hits
denies
hero_damage
tower_damage
net_worth
items
skill_build
created_at
played_at
```

Do not store only raw match JSON and depend on external APIs forever.

Persist the normalized information needed for analytics.

---

# 11. Deterministic Metrics Engine

Metrics must be calculated by backend code.

Do NOT ask the LLM to perform arithmetic.

Examples:

```text
GPM
XPM
KDA
Deaths / 10 minutes
Kills / 10 minutes
Last hits @10
Last hits @15
Net worth @10
Net worth @15
XP @10
Gold advantage @10
Hero damage
Tower damage
Building damage
Kill participation
Objective participation
First major item timing
BKB timing
Blink timing
Midas timing
First rotation
```

Metrics must be deterministic and reproducible.

The LLM should interpret metrics, not calculate them.

---

# 12. Benchmark Engine

Benchmarking is a first-class domain component.

The goal is to answer:

> How does this player compare to appropriate players?

Benchmark context must consider:

```text
Hero
Role
Rank / MMR Bracket
Patch
Metric
```

Example:

```text
Player:
GPM = 521

Peer Average:
505

Top 20%:
548

Percentile:
67

Gap to Top 20%:
27
```

Benchmark result should contain:

```text
metric
player_value
peer_average
top_percentile_value
percentile
sample_size
patch
hero
role
rank_bracket
```

Do NOT make percentile claims when the sample size is insufficient.

Benchmarking must be patch-aware.

---

# 13. Benchmark Provider

Create:

```rust
trait BenchmarkProvider {
    async fn get_benchmark(
        &self,
        context: BenchmarkContext,
    ) -> Result<BenchmarkResult>;
}
```

The provider implementation should be replaceable.

The benchmark engine should not be tied to a specific website.

---

# 14. Hero Pool

The system must maintain a model of the player's Hero Pool.

For every hero the player has meaningful experience with, track:

```text
matches_played
wins
losses
win_rate
recent_matches
recent_win_rate
role
average_metrics
benchmark_performance
confidence
last_played_at
```

Hero pool should eventually classify heroes into categories such as:

```text
Signature
Comfort
Stretch
Risk
```

Classification must be based on actual player history rather than arbitrary labels.

---

# 15. HERO INTELLIGENCE

This is a major product feature.

Do NOT implement this as a simple "highest win-rate hero" recommendation.

The product should answer:

> "Among the heroes that are currently strong, which ones are actually a good fit for this player?"

Architecture:

```text
             Dota Meta Data
                    ↓
              HeroMetaProvider
                    ↓
              Hero Meta Model
                    │
       ┌────────────┴────────────┐
       ↓                         ↓
 Player Hero Pool          Player Benchmark
       │                         │
       └────────────┬────────────┘
                    ↓
            Hero Intelligence
                    ↓
              Hero Fit Score
                    ↓
         Personalized Recommendation
                    ↓
                 AI Coach
```

---

# 16. Hero Meta Providers

Do NOT tightly couple the system to Dotabuff.

The initial provider priority should be:

```text
1. STRATZ
2. OpenDota fallback
3. Dotabuff optional provider
```

STRATZ should be the preferred source for richer rank/role/hero meta data where available.

OpenDota should remain a fallback and already fits the project's provider architecture.

Dotabuff should NOT be required for the core application.

If Dotabuff data is used, isolate it completely behind:

```rust
struct DotabuffHeroMetaProvider;
```

The system must continue functioning if Dotabuff becomes unavailable.

Before implementing a provider, verify the current availability, API/access method, rate limits, and usage constraints of the external source.

Do not build the core architecture around HTML scraping.

---

# 17. HeroMetaProvider

Create:

```rust
trait HeroMetaProvider {
    async fn get_hero_meta(
        &self,
        context: HeroMetaContext,
    ) -> Result<Vec<HeroMeta>>;
}
```

Context should be capable of representing:

```text
patch
rank / MMR bracket
role
time window
```

Example:

```rust
struct HeroMetaContext {
    patch: Patch,
    rank: RankBracket,
    role: Role,
    time_window: TimeWindow,
}
```

Hero meta should contain data such as:

```text
hero
pick_rate
win_rate
ban_rate
role
rank_bracket
patch
sample_size
meta_strength
```

Only include fields that the actual provider can reliably supply.

Do not invent data.

---

# 18. Meta Strength

Do not equate:

```text
Meta Strength = Win Rate
```

Meta strength can combine available signals such as:

```text
Win Rate
Pick Rate
Ban Rate
Rank-specific performance
Role-specific performance
Recent trend
Sample size
```

The scoring algorithm must be deterministic and explainable.

---

# 19. Hero Fit Score

Create an explainable scoring model.

Initial scoring can consider:

```text
Player Performance
Current Meta Strength
Player Experience
Benchmark Performance
Recent Form
Training Focus Compatibility
```

Example initial weighting:

```text
30% Player Performance
25% Meta Strength
20% Player Experience
15% Benchmark Performance
10% Recent Form
```

Training Focus compatibility may initially be used as a modifier rather than another independent weighted component.

Do not hard-code these weights into the architecture.

Make them configurable so they can evolve after real-world data.

---

# 20. Hero Recommendation Categories

Possible recommendation levels:

```text
Recommended
Consider
Avoid for Now
```

Example:

```text
Puck — Recommended

Meta Strength: 88
Your Performance: 82
Experience: 91
Benchmark: 79
Recent Form: 84

Fit Score: 85
```

The explanation should say something like:

```text
Puck is currently strong in your rank and role.
You already have significant experience on the hero,
your performance is above the relevant benchmark,
and the hero aligns with your current training focus.
```

The exact wording should be generated by the coaching/LLM layer, but the underlying scores must be deterministic.

---

# 21. Important Recommendation Rule

Meta recommendation must NEVER completely override the player's long-term coaching context.

The goal is:

```text
Improve the player
```

not:

```text
Abuse the current meta
```

For example:

If a hero has a high global win rate but:

- player has almost no experience
- player performs poorly on it
- player has poor recent results

do not automatically recommend it.

Likewise, a hero with slightly lower meta strength can be a better recommendation if:

- player is highly experienced
- player performs well
- benchmark results are strong
- hero fits current Training Focus

---

# 22. Training Focus + Hero Intelligence

Hero recommendations should eventually interact with Training Focus.

Example:

```text
Current Training Focus:
Improve teamfight positioning
```

Suppose:

```text
Puck
```

is:

- strong in current meta
- one of the player's strongest heroes
- provides opportunities to practice the relevant skill

Then the system can recommend Puck partly because it aligns with the current training objective.

Therefore:

```text
Current Meta
+
Hero Pool
+
Benchmark
+
Player Model
+
Training Focus
=
Hero Recommendation
```

---

# 23. Future Draft Intelligence

Do not implement full draft intelligence initially.

But design the architecture so it can later support:

```text
Your Hero Pool
+
Current Meta
+
Enemy Heroes
+
Your Team
+
Training Focus
↓
Recommended Hero
```

This should be a future extension of Hero Intelligence rather than a separate architecture.

---

# 24. Player Model

The system must maintain a long-term model of the player.

Example:

```text
PlayerModel
├── strengths
├── weaknesses
├── hero_pool
├── preferred_roles
├── recurring_patterns
├── benchmark_profile
├── recent_form
├── training_history
└── confidence
```

The Player Model should evolve as new matches arrive.

Do not rebuild the player's identity from scratch for every match.

---

# 25. Recurring Pattern Detection

The system should detect patterns across multiple matches.

Examples:

```text
Dies too often before first major item
Low CS at 10 minutes
Strong laning but poor mid-game conversion
Too many unnecessary deaths
Low teamfight participation
Poor objective participation
Late BKB timing
Excellent farming but low fight participation
Repeatedly loses advantage after winning lane
```

A pattern should require sufficient evidence.

Do not call something a "recurring pattern" after one match.

---

# 26. AI Coaching

The LLM should receive structured deterministic data.

Example:

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

The LLM should answer:

```text
What happened?
Why does it matter?
What pattern is emerging?
What should the player do differently?
```

The LLM must NOT calculate statistics.

---

# 27. Coaching Insights

Create coaching insights such as:

```text
Strength
Weakness
Recurring Pattern
Recommendation
Warning
Improvement
```

Every insight should ideally reference measurable evidence.

Bad:

```text
You need to farm better.
```

Good:

```text
Your average last hits at 10 minutes are 43,
compared with 51 for players in your benchmark group.
This gap appears in 7 of your last 10 games.
```

---

# 28. Training Focus

The system should identify **one primary training focus** at a time.

Do not overwhelm the user with 10 weaknesses.

Example:

```text
CURRENT TRAINING FOCUS

Reduce unnecessary deaths during mid game.

Why:

You died 6.1 times per game during minutes 10–25
over your last 10 games.

Benchmark:
4.3 deaths.

This pattern appeared in 7/10 matches.

Goal:
Reduce this to below 5 deaths per game.
```

Training Focus should be selected using:

```text
Benchmark Gap
+
Historical Pattern
+
Recent Performance
+
Impact
+
Confidence
+
Recency
```

Do NOT simply select the lowest statistic.

---

# 29. Progress Tracking

Track progress over time.

Example:

```text
GPM percentile:
42 → 48 → 55 → 61
```

Other examples:

```text
Deaths / 10
LH@10
Net Worth@15
Hero Damage
Kill Participation
Objective Participation
```

The user should be able to see whether their training focus is actually improving.

---

# 30. Dashboard

The dashboard should prioritize coaching.

Recommended order:

```text
Current Training Focus
        ↓
Why This Matters
        ↓
Progress
        ↓
Hero Recommendation
        ↓
Recent Matches
        ↓
Benchmark Summary
```

The dashboard should NOT primarily look like a statistics spreadsheet.

The main question should be:

> "What should I do next?"

---

# 31. Benchmark Page

Create a dedicated benchmark view.

Show:

```text
Your Value
Peer Average
Top 20%
Percentile
Gap
Sample Size
Patch
Hero
Role
Rank
```

Use visualizations where appropriate.

---

# 32. Hero Intelligence Page

Create a dedicated page/feature for Hero Intelligence.

Possible route:

```text
/heroes
```

or:

```text
/hero-intelligence
```

Show:

```text
Recommended Heroes
Current Meta
Your Hero Pool
Hero Fit Score
Why This Hero?
Benchmark
Recent Form
Experience
```

Example:

```text
Recommended for You

1. Puck
   Fit: 85
   Meta: Strong
   Personal Performance: Excellent

2. Storm Spirit
   Fit: 78
   Meta: Strong
   Personal Performance: Good

3. Invoker
   Fit: 64
   Meta: Strong
   Personal Performance: Average
```

The UI should make it obvious that recommendations are personalized.

---

# 33. API

Required API structure:

```text
GET  /api/auth/steam
GET  /api/auth/steam/callback
GET  /api/auth/me
POST /api/auth/logout

GET  /api/players/me
POST /api/players/me/sync

GET  /api/matches
GET  /api/matches/:id
POST /api/matches/:id/analyze

GET  /api/stats

GET  /api/benchmark
GET  /api/benchmark/:metric

GET  /api/heroes
GET  /api/heroes/recommendations
GET  /api/hero-intelligence

GET  /api/coach
GET  /api/coach/training-focus

GET  /api/billing
GET  /api/billing/subscription
GET  /api/billing/payments
POST /api/billing/checkout
POST /api/billing/webhook
```

Authentication and entitlement middleware must protect appropriate endpoints.

---

# 34. Database Architecture

Target structure:

```text
users
 ├── sessions
 ├── dota_players
 │    └── matches
 │         ├── match_metrics
 │         ├── benchmark_results
 │         └── coaching_insights
 │
 ├── player_patterns
 ├── training_focus
 │
 ├── hero_pool
 ├── hero_recommendations
 │
 └── subscriptions
      └── payments
```

Meta data may be persisted through snapshots/cache tables if needed:

```text
hero_meta_snapshots
```

A snapshot can include:

```text
hero
patch
rank_bracket
role
time_window
provider
metrics
sample_size
created_at
```

Do not persist data unnecessarily if it can safely be cached.

---

# 35. Backend Structure

Target modular structure:

```text
backend/
├── auth/
│   ├── steam/
│   ├── sessions/
│   └── middleware/
│
├── users/
│
├── dota/
│   ├── providers/
│   ├── players/
│   └── matches/
│
├── metrics/
│
├── benchmarks/
│   ├── domain/
│   ├── providers/
│   └── service/
│
├── heroes/
│   ├── meta/
│   │   ├── providers/
│   │   └── service/
│   │
│   ├── pool/
│   └── intelligence/
│
├── player_model/
│
├── patterns/
│
├── coaching/
│   ├── insights/
│   ├── training_focus/
│   └── llm/
│
├── billing/
│   ├── subscriptions/
│   ├── payments/
│   └── providers/
│
├── repositories/
│
├── database/
│
└── api/
```

Adapt this to the existing repository instead of mechanically creating duplicate modules.

---

# 36. Frontend Structure

Use feature-oriented organization:

```text
features/
├── auth/
├── dashboard/
├── matches/
├── benchmark/
├── heroes/
├── hero-intelligence/
├── coach/
├── billing/
└── account/
```

Reuse the existing architecture where appropriate.

Do not rewrite existing frontend code simply to match this structure if the current structure is already good.

---

# 37. Landing Page

Core message:

```text
Your Personal Dota 2 Coach

Connect your Steam account.
Let AI learn how you play.
Improve one weakness at a time.

Start 14-Day Free Trial

Then $9.98/month
```

The exact copy can be refined later.

---

# 38. PWA

The application should remain PWA-compatible.

Requirements:

- Manifest
- Installability
- Responsive UI
- Mobile-friendly dashboard
- Appropriate icons
- Offline support where useful

Do not compromise backend security or data correctness for PWA behavior.

---

# 39. LLM Architecture

Use an abstraction:

```rust
trait LlmProvider {
    async fn generate(...);
}
```

The application should support an OpenAI-compatible provider initially.

The LLM layer should receive structured domain data.

Never let the LLM become the source of truth for:

- Match statistics
- Benchmark values
- Percentiles
- Payment state
- Subscription state
- Trial state
- Steam identity

---

# 40. Caching and External APIs

External providers should not be called unnecessarily.

Implement appropriate caching/synchronization.

Examples:

```text
Match data → cache/persist locally
Hero meta → cache by patch/rank/role/time window
Benchmark → cache when appropriate
```

Respect provider rate limits.

Provider failures should not crash the entire application.

---

# 41. Error Handling

Every provider should have clear errors.

Example:

```text
ProviderUnavailable
RateLimited
InvalidResponse
AuthenticationFailed
NotFound
InsufficientData
```

The coaching engine should gracefully handle incomplete external data.

For example:

```text
Hero meta unavailable
```

should not make the entire dashboard unusable.

---

# 42. Data Quality

Never fabricate:

- hero statistics
- benchmarks
- percentile values
- player metrics
- rank
- match data

If data is insufficient, explicitly represent that.

Example:

```text
Insufficient sample size for reliable benchmark.
```

This is better than producing a false percentile.

---

# 43. Security

Important:

- Steam identity must be backend-derived
- Session must be server-side
- Cookies must be secure
- Billing must be backend-controlled
- Webhooks must be verified
- Subscription status must never be trusted from frontend
- Trial expiration must be calculated server-side
- Secrets must come from environment variables
- Provider API keys must never reach frontend
- Validate all external API responses
- Protect authenticated routes
- Prevent IDOR between users

---

# 44. Observability

Prepare the backend for:

```text
structured logging
request IDs
provider errors
payment events
sync events
LLM errors
benchmark failures
```

Do not add unnecessary infrastructure yet.

Keep this compatible with a modular monolith.

---

# 45. Testing

At minimum add tests for:

### Authentication

```text
Steam callback
Session creation
Session expiration
Logout
```

### Billing

```text
Trial creation
Trial expiration
Payment creation
Webhook verification
Duplicate webhook
Subscription activation
```

### Metrics

```text
GPM
XPM
KDA
LH@10
Deaths/10
```

### Benchmark

```text
Percentile
Insufficient sample size
Hero/role/rank/patch segmentation
```

### Hero Intelligence

```text
Hero Fit Score
Insufficient hero experience
Meta + personal performance interaction
Training Focus compatibility
Recommendation ordering
```

### Coaching

```text
Recurring pattern detection
Training Focus selection
Progress calculation
```

---

# 46. Development Phases

Implement incrementally.

## Phase 1 — Repository Assessment & Foundation

First inspect the existing repository.

Do not immediately rewrite code.

Provide:

```text
Current architecture
What already works
What is missing
What should be preserved
What should be refactored
Potential architectural risks
```

Then implement only the necessary foundation changes.

---

## Phase 2 — Steam Authentication

Implement:

```text
Steam OpenID
Users
Sessions
Auth middleware
Login/logout
```

Definition of done:

User can log in through Steam and receive a secure authenticated session.

---

## Phase 3 — Dota Integration

Implement:

```text
DotaProvider
OpenDota integration
Player resolution
Match synchronization
Match persistence
```

---

## Phase 4 — Deterministic Analytics

Implement:

```text
Match metrics
Aggregated player statistics
Hero statistics
Role statistics
```

All calculations must happen in Rust/backend.

---

## Phase 5 — Benchmark Engine

Implement:

```text
BenchmarkProvider
Benchmark context
Rank/role/hero/patch segmentation
Percentiles
Top 20%
Sample-size validation
Benchmark API
```

---

## Phase 6 — Hero Intelligence

Implement:

```text
HeroMetaProvider
STRATZ provider
OpenDota fallback
Optional Dotabuff provider
Hero meta model
Hero pool
Hero Fit Score
Hero recommendations
Hero Intelligence API
Hero Intelligence UI
```

Important:

Do not make Dotabuff a hard dependency.

The system must remain functional if Dotabuff is unavailable.

---

## Phase 7 — AI Coach

Implement:

```text
LLM provider
Coaching insights
Player analysis
Evidence-based explanations
```

---

## Phase 8 — Player Model & Recurring Patterns

Implement:

```text
Player Model
Recurring Pattern Detection
Historical analysis
Hero confidence
Recent form
```

---

## Phase 9 — Training Focus & Progress

Implement:

```text
Training Focus
Goal tracking
Benchmark progress
Historical progress
```

---

## Phase 10 — Trial & Subscription

Implement:

```text
14-day trial
Entitlements
Subscription state
Crypto checkout
Payment persistence
Webhooks
Idempotency
```

---

## Phase 11 — PWA & Launch

Implement:

```text
PWA
Responsive UI
Landing page
Production configuration
Error states
Loading states
Basic observability
```

---

# 47. Important Scope Rule

Do NOT prematurely implement:

```text
Microservices
Live game overlay
Voice coaching
Replay parsing
Native mobile applications
Social network
Leaderboards
Complex team features
Full draft assistant
```

These may be future products/features.

Focus on making the core loop excellent:

```text
Player
→ Matches
→ Metrics
→ Benchmark
→ Hero Intelligence
→ Patterns
→ Coach
→ Training Focus
→ Progress
```

---

# 48. Product Differentiation

Do not build a generic "AI Dota analyzer".

The long-term differentiation should be:

```text
Persistent Player Model
+
Recurring Patterns
+
Personal Benchmarking
+
Hero Intelligence
+
Training Focus
+
Progress Tracking
```

The system should become better at understanding the player over time.

A new user should receive generic analysis.

A player with 100 matches should receive increasingly personalized coaching.

---

# 49. Engineering Rule for Claude Code

Before modifying code:

1. Inspect the repository.
2. Understand the current architecture.
3. Identify what is already implemented.
4. Identify the smallest safe change.
5. Do not rewrite working modules unnecessarily.
6. Reuse existing abstractions where possible.
7. Keep provider-specific code isolated.
8. Add tests for important domain logic.
9. Run the relevant tests/build after changes.
10. Explain what changed and why.

Do not create fake implementations just to satisfy the API.

If an external API is unavailable or requires credentials, implement the correct abstraction and integration boundary rather than pretending real data exists.

---

# 50. First Task

Your FIRST task is NOT to implement the entire specification.

First:

### Step 1

Inspect the repository.

### Step 2

Map the current architecture to this specification.

### Step 3

Create an implementation gap analysis:

```text
Already implemented
Partially implemented
Missing
Needs refactor
Potential technical debt
```

### Step 4

Identify the best implementation order based on the existing codebase.

### Step 5

Only after this assessment, begin implementation phase-by-phase.

Do not destroy existing working code.

The final architecture should be production-oriented, modular, testable, provider-independent, and capable of evolving into a serious Dota 2 coaching SaaS.
