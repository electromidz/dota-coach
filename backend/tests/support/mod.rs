//! Test harness: the real router, with the two external dependencies stubbed.
//!
//! Nothing here reaches OpenDota or Valve. The database is real, because the
//! behaviour under test (uniqueness, ownership scoping, pagination) lives in
//! SQL and would be meaningless against a fake.

// Each test binary compiles this module separately and uses a different part of
// it, so anything one of them does not touch reads as dead code there. The
// alternative is annotating half the file.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use chrono::{DateTime, Duration, TimeZone, Utc};
use dota_coach_backend::api;
use dota_coach_backend::config::{
    AuthConfig, BillingConfig, CalibrationConfig, CoachConfig, Config, DotaConfig, HeroConfig,
    LlmConfig, RoleConfig, TrainingConfig,
};
use dota_coach_backend::domain::benchmark::{
    BenchmarkContext, BenchmarkMetric, Bucket, ResolvedBracket, Segment,
};
use dota_coach_backend::domain::billing::PaymentStatus;
use dota_coach_backend::domain::hero::FitWeights;
use dota_coach_backend::domain::hero::{HeroMeta, HeroMetaContext, RankBracket};
use dota_coach_backend::domain::r#match::NormalizedMatch;
use dota_coach_backend::domain::role::RoleScoreWeights;
use dota_coach_backend::domain::session::{hash_token, NewToken};
use dota_coach_backend::domain::training::FocusWeights;
use dota_coach_backend::services::auth::steam_openid::{OpenIdError, SteamOpenId, SteamVerifier};
use dota_coach_backend::services::benchmarks::{BenchmarkError, BenchmarkProvider, Distribution};
use dota_coach_backend::services::dota::{DotaDataProvider, ProviderError, ProviderPlayer};
use dota_coach_backend::services::hero_meta::strength::MetaWeights;
use dota_coach_backend::services::hero_meta::{HeroMetaError, HeroMetaProvider, HeroMetaSet};
use dota_coach_backend::services::llm::{LlmCompletion, LlmError, LlmProvider, LlmRequest};
use dota_coach_backend::services::payments::{
    CheckoutRequest, CheckoutSession, PaymentError, PaymentProvider, PaymentUpdate,
};
use dota_coach_backend::state::{AppState, Providers};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

/// SteamID64 base; test accounts are built from it so they are always valid.
const STEAM_ID64_BASE: i64 = 76_561_197_960_265_728;

// ---------------------------------------------------------------------------
// Mock Dota provider
// ---------------------------------------------------------------------------

/// Which provider call should fail, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Unavailable,
    RateLimited,
    NotFound,
    InvalidResponse,
}

impl Failure {
    fn as_error(self) -> ProviderError {
        match self {
            Failure::Unavailable => ProviderError::Unavailable("connection refused".into()),
            Failure::RateLimited => ProviderError::RateLimited,
            Failure::NotFound => ProviderError::NotFound,
            Failure::InvalidResponse => ProviderError::Decode("unexpected shape".into()),
        }
    }
}

#[derive(Default)]
pub struct MockDota {
    matches: Mutex<Vec<NormalizedMatch>>,
    list_failure: Mutex<Option<Failure>>,
    detail_failure: Mutex<Option<Failure>>,
    pub list_calls: AtomicUsize,
    pub detail_calls: AtomicUsize,
}

impl MockDota {
    pub fn with_matches(matches: Vec<NormalizedMatch>) -> Arc<Self> {
        Arc::new(Self {
            matches: Mutex::new(matches),
            ..Default::default()
        })
    }

    pub fn failing(failure: Failure) -> Arc<Self> {
        Arc::new(Self {
            list_failure: Mutex::new(Some(failure)),
            ..Default::default()
        })
    }

    /// Match-detail calls fail while the match list still succeeds: a sync
    /// should degrade rather than abort.
    pub fn with_failing_details(matches: Vec<NormalizedMatch>, failure: Failure) -> Arc<Self> {
        Arc::new(Self {
            matches: Mutex::new(matches),
            detail_failure: Mutex::new(Some(failure)),
            ..Default::default()
        })
    }

    pub fn set_matches(&self, matches: Vec<NormalizedMatch>) {
        *self.matches.lock().unwrap() = matches;
    }
}

#[async_trait]
impl DotaDataProvider for MockDota {
    async fn get_player(&self, account_id: i64) -> Result<ProviderPlayer, ProviderError> {
        Ok(ProviderPlayer {
            account_id,
            persona_name: Some("Test Persona".into()),
            avatar_url: Some("https://avatars.example/full.jpg".into()),
            profile_url: Some("https://steamcommunity.com/id/test/".into()),
            rank_tier: Some(55),
            // Legend 5 is nowhere near the Immortal ladder, so the honest
            // fixture value is "not on it".
            leaderboard_rank: None,
            has_public_profile: true,
        })
    }

    async fn get_player_matches(
        &self,
        _account_id: i64,
        limit: u32,
    ) -> Result<Vec<NormalizedMatch>, ProviderError> {
        self.list_calls.fetch_add(1, Ordering::SeqCst);

        if let Some(failure) = *self.list_failure.lock().unwrap() {
            return Err(failure.as_error());
        }

        let matches = self.matches.lock().unwrap();
        Ok(matches.iter().take(limit as usize).cloned().collect())
    }

    async fn get_match_details(
        &self,
        match_id: i64,
        _account_id: i64,
    ) -> Result<NormalizedMatch, ProviderError> {
        self.detail_calls.fetch_add(1, Ordering::SeqCst);

        if let Some(failure) = *self.detail_failure.lock().unwrap() {
            return Err(failure.as_error());
        }

        let matches = self.matches.lock().unwrap();
        let mut detail = matches
            .iter()
            .find(|m| m.match_id == match_id)
            .cloned()
            .ok_or(ProviderError::NotFound)?;

        // The real detail endpoint carries what the summary cannot.
        detail.denies = Some(10);
        detail.net_worth = Some(20_000);
        detail.from_details = true;
        Ok(detail)
    }

    async fn heroes(&self) -> Result<HashMap<i32, String>, ProviderError> {
        Ok(HashMap::from([
            (35, "Luna".to_string()),
            (5, "Crystal Maiden".to_string()),
        ]))
    }
}

// ---------------------------------------------------------------------------
// Stub Steam verifier
// ---------------------------------------------------------------------------

/// Stands in for Valve. Returns a fixed Steam id, or refuses.
pub struct StubVerifier {
    steam_id: Option<i64>,
    unavailable: bool,
}

impl StubVerifier {
    pub fn accepting(steam_id: i64) -> Arc<Self> {
        Arc::new(Self {
            steam_id: Some(steam_id),
            unavailable: false,
        })
    }

    pub fn rejecting() -> Arc<Self> {
        Arc::new(Self {
            steam_id: None,
            unavailable: false,
        })
    }

    pub fn unavailable() -> Arc<Self> {
        Arc::new(Self {
            steam_id: None,
            unavailable: true,
        })
    }
}

#[async_trait]
impl SteamVerifier for StubVerifier {
    async fn verify(&self, _params: &BTreeMap<String, String>) -> Result<i64, OpenIdError> {
        if self.unavailable {
            return Err(OpenIdError::Unavailable("stub offline".into()));
        }
        self.steam_id.ok_or(OpenIdError::InvalidAssertion)
    }
}

// ---------------------------------------------------------------------------
// Stub benchmark provider
// ---------------------------------------------------------------------------

/// Stands in for OpenDota's `/benchmarks`. Returns a fixed distribution, or
/// refuses, so the engine's honesty rules can be tested without a network.
pub struct StubBenchmarks {
    failure: Option<&'static str>,
    /// Brackets this stub publishes nothing for.
    ///
    /// The real provider answers a thin bracket with a full bucket list of
    /// nulls, which the parser turns into an empty distribution and the caller
    /// retries at all ranks. This reproduces the *outcome* of that — a request
    /// for a bracket with no data coming back as a recorded fallback — without
    /// reproducing OpenDota's payload shape, which has its own unit tests.
    empty_brackets: Vec<RankBracket>,
}

impl StubBenchmarks {
    /// A distribution whose gold-per-minute median is 500 and top-20% is 800.
    pub fn serving() -> Arc<Self> {
        Arc::new(Self {
            failure: None,
            empty_brackets: Vec::new(),
        })
    }

    /// Serving, except for one bracket it has no data for.
    pub fn without_bracket(bracket: RankBracket) -> Arc<Self> {
        Arc::new(Self {
            failure: None,
            empty_brackets: vec![bracket],
        })
    }

    pub fn unavailable() -> Arc<Self> {
        Arc::new(Self {
            failure: Some("offline"),
            empty_brackets: Vec::new(),
        })
    }
}

#[async_trait]
impl BenchmarkProvider for StubBenchmarks {
    async fn get_distribution(
        &self,
        context: &BenchmarkContext,
    ) -> Result<Distribution, BenchmarkError> {
        if let Some(reason) = self.failure {
            return Err(BenchmarkError::Unavailable(reason.to_string()));
        }

        let buckets = |lo: f32, mid: f32, hi: f32| {
            vec![
                Bucket {
                    percentile: 0.1,
                    value: lo,
                },
                Bucket {
                    percentile: 0.5,
                    value: mid,
                },
                Bucket {
                    percentile: 0.8,
                    value: hi,
                },
                Bucket {
                    percentile: 0.9,
                    value: hi * 1.2,
                },
            ]
        };

        // Resolved the same way the real provider does, so the integration
        // tests exercise the rank-segmentation path rather than a stub that
        // quietly always says "hero only" — including an explicitly requested
        // bracket winning over the one the player's rank implies.
        let requested = match context.bracket {
            Some(bracket) => ResolvedBracket::exact(bracket),
            None => ResolvedBracket::requested_for(context.rank_tier),
        };

        let bracket = match requested.used {
            Some(asked) if self.empty_brackets.contains(&asked) => {
                ResolvedBracket::fell_back_from(asked)
            }
            _ => requested,
        };

        // Higher brackets farm faster. Applied only to an *explicitly asked
        // for* bracket, so every existing expectation about the rank-derived
        // path — median 500, top-20% 800 — still holds exactly, while a test
        // that switches bracket can prove the numbers moved with it.
        let shift = match (context.bracket, bracket.used) {
            (Some(_), Some(used)) => used.index() as f32 * 20.0,
            _ => 0.0,
        };

        Ok(Distribution {
            buckets: HashMap::from([
                (
                    BenchmarkMetric::GoldPerMin,
                    buckets(200.0 + shift, 500.0 + shift, 800.0 + shift),
                ),
                (
                    BenchmarkMetric::XpPerMin,
                    buckets(250.0 + shift, 550.0 + shift, 850.0 + shift),
                ),
                (BenchmarkMetric::DeathsPerMin, buckets(0.05, 0.15, 0.30)),
            ]),
            segmented_by: if bracket.is_rank_segmented() {
                vec![Segment::Hero, Segment::RankBracket]
            } else {
                vec![Segment::Hero]
            },
            sample_size: None,
            bracket,
        })
    }
}

// ---------------------------------------------------------------------------
// Stub hero meta provider
// ---------------------------------------------------------------------------

/// Stands in for OpenDota's `/heroStats`.
///
/// Meta strengths are set directly rather than scored, so a test asserting on
/// a recommendation is not also asserting on the strength formula — that has
/// its own unit tests.
pub struct StubHeroMeta {
    failure: Option<&'static str>,
}

impl StubHeroMeta {
    /// Luna strong, Crystal Maiden weak — the two heroes `MockDota` knows.
    /// Puck is a hero no fixture player has ever touched.
    pub fn serving() -> Arc<Self> {
        Arc::new(Self { failure: None })
    }

    pub fn unavailable() -> Arc<Self> {
        Arc::new(Self {
            failure: Some("offline"),
        })
    }
}

#[async_trait]
impl HeroMetaProvider for StubHeroMeta {
    async fn get_hero_meta(&self, context: &HeroMetaContext) -> Result<HeroMetaSet, HeroMetaError> {
        if let Some(reason) = self.failure {
            return Err(HeroMetaError::Unavailable(reason.to_string()));
        }

        let hero = |id: i32, name: &str, win_rate: f32, strength: f32| HeroMeta {
            hero_id: id,
            hero_name: name.to_string(),
            roles: vec!["Carry".to_string()],
            picks: 50_000,
            wins: (50_000.0 * win_rate) as i64,
            win_rate,
            pick_rate: 0.05,
            trend: Some(0.0),
            meta_strength: strength,
            bracket: context.bracket,
        };

        Ok(HeroMetaSet {
            heroes: vec![
                hero(35, "Luna", 0.54, 80.0),
                hero(5, "Crystal Maiden", 0.47, 30.0),
                hero(13, "Puck", 0.55, 95.0),
            ],
            segmented_by: context
                .bracket
                .map(|_| vec![Segment::RankBracket])
                .unwrap_or_default(),
            bracket: context.bracket,
            source: "StubMeta",
            note: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Stub LLM provider
// ---------------------------------------------------------------------------

/// Stands in for the coaching model.
///
/// Records every call, so a test can prove that a cached analysis costs no
/// model call at all rather than merely producing the same text.
#[derive(Default)]
pub struct StubLlm {
    pub calls: AtomicUsize,
    answer: Option<String>,
    failure: Option<&'static str>,
    configured: bool,
}

impl StubLlm {
    /// Answers with a valid analysis citing evidence every player has.
    pub fn answering() -> Arc<Self> {
        Self::with_answer(
            r#"{
                "summary": "You win more than you lose, and your farm is the thing holding you back.",
                "insights": [
                    {
                        "kind": "weakness",
                        "title": "Your farm trails the hero's peers",
                        "explanation": "Closing the gap is the single biggest lever you have right now.",
                        "evidence": ["overall.record"]
                    }
                ]
            }"#,
        )
    }

    pub fn with_answer(answer: &str) -> Arc<Self> {
        Arc::new(Self {
            answer: Some(answer.to_string()),
            configured: true,
            ..Default::default()
        })
    }

    /// Configured, but the endpoint is down.
    pub fn unavailable() -> Arc<Self> {
        Arc::new(Self {
            failure: Some("offline"),
            configured: true,
            ..Default::default()
        })
    }

    /// No API key on this deployment.
    pub fn unconfigured() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl LlmProvider for StubLlm {
    fn is_configured(&self) -> bool {
        self.configured
    }

    async fn generate(&self, _request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        self.calls.fetch_add(1, Ordering::SeqCst);

        if !self.configured {
            return Err(LlmError::NotConfigured);
        }
        if let Some(reason) = self.failure {
            return Err(LlmError::Unavailable(reason.to_string()));
        }

        Ok(LlmCompletion {
            text: self.answer.clone().unwrap_or_default(),
            model: "stub-model".to_string(),
            input_tokens: Some(100),
            output_tokens: Some(50),
        })
    }
}

// ---------------------------------------------------------------------------
// Mock payment provider
// ---------------------------------------------------------------------------

/// The signature this stub accepts. Real HMAC verification is NOWPayments'
/// own problem and is unit-tested there; what these tests need is a provider
/// that can say yes or no, so the billing rules around it are exercised
/// without a vendor's crypto in the way.
pub const VALID_SIGNATURE: &str = "valid-signature";

/// Stands in for a crypto payment provider.
///
/// Records every charge it opens, so a test can prove that a second checkout
/// reuses the first invoice rather than merely returning something similar.
pub struct StubPayments {
    pub created: AtomicUsize,
    configured: bool,
    /// What `get_payment_status` reports, when a test wants the polling path.
    polled_status: Mutex<Option<PaymentUpdate>>,
}

impl StubPayments {
    pub fn taking_payments() -> Arc<Self> {
        Arc::new(Self {
            created: AtomicUsize::new(0),
            configured: true,
            polled_status: Mutex::new(None),
        })
    }

    /// A deployment with no payment credentials at all.
    pub fn unconfigured() -> Arc<Self> {
        Arc::new(Self {
            created: AtomicUsize::new(0),
            configured: false,
            polled_status: Mutex::new(None),
        })
    }
}

#[async_trait]
impl PaymentProvider for StubPayments {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn is_configured(&self) -> bool {
        self.configured
    }

    fn signature_header(&self) -> &'static str {
        "x-stub-signature"
    }

    async fn create_payment(
        &self,
        request: &CheckoutRequest,
    ) -> Result<CheckoutSession, PaymentError> {
        if !self.configured {
            return Err(PaymentError::NotConfigured);
        }

        self.created.fetch_add(1, Ordering::SeqCst);

        Ok(CheckoutSession {
            // Unique per charge: tests share a database, and the provider id is
            // unique across every row in it.
            provider_payment_id: format!("stub-payment-{}", Uuid::new_v4()),
            payment_url: Some(format!("https://pay.example/i/{}", request.order_id)),
            status: PaymentStatus::Pending,
        })
    }

    async fn get_payment_status(
        &self,
        provider_payment_id: &str,
    ) -> Result<PaymentUpdate, PaymentError> {
        self.polled_status.lock().unwrap().clone().ok_or_else(|| {
            PaymentError::Unavailable(format!("no stub status for {provider_payment_id}"))
        })
    }

    /// Accepts exactly one signature, then reads the body the same way a real
    /// provider's parser would.
    fn handle_webhook(
        &self,
        signature: Option<&str>,
        body: &[u8],
    ) -> Result<PaymentUpdate, PaymentError> {
        if signature != Some(VALID_SIGNATURE) {
            return Err(PaymentError::InvalidSignature);
        }

        let body: serde_json::Value = serde_json::from_slice(body)
            .map_err(|e| PaymentError::InvalidResponse(e.to_string()))?;

        let provider_payment_id = body["payment_id"].as_str().unwrap_or_default().to_string();
        let status = PaymentStatus::parse(body["status"].as_str().unwrap_or_default())
            .ok_or_else(|| PaymentError::InvalidResponse("unknown status".into()))?;

        Ok(PaymentUpdate {
            event_key: format!("{provider_payment_id}:{}", status.slug()),
            provider_payment_id,
            order_id: body["order_id"].as_str().map(str::to_string),
            status,
            amount_cents: body["amount_cents"].as_i64(),
            currency: body["currency"].as_str().map(str::to_string),
            pay_currency: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

pub struct TestApp {
    pub router: Router,
    pub db: PgPool,
}

pub fn test_config() -> Config {
    Config {
        database_url: String::new(),
        host: "127.0.0.1".into(),
        port: 0,
        cors_origins: vec!["http://localhost:3000".into()],
        auth: AuthConfig {
            public_base_url: "http://localhost:8080".into(),
            frontend_base_url: "http://localhost:3000".into(),
            steam_openid_url: "https://steamcommunity.example/openid/login".into(),
            session_ttl_hours: 720,
            cookie_secure: false,
            cookie_cross_site: false,
        },
        dota: DotaConfig {
            base_url: "https://dota.example/api".into(),
            api_key: None,
            // Above any fixture size, so pagination tests are not silently
            // truncated by the sync cap. `sync_match_limit` has its own test.
            sync_match_limit: 100,
            // Disabled by default so a test can sync twice in a row; the
            // cooldown itself is unit-tested.
            sync_cooldown_seconds: 0,
            request_timeout_seconds: 5,
            benchmark_ttl_hours: 24,
            significant_only: false,
        },
        coach: CoachConfig {
            // Disabled by default so a test can analyse twice in a row; the
            // limiter has its own test that turns it back on.
            cooldown_seconds: 0,
            daily_limit: 20,
            max_insights: 5,
            max_plan_steps: 4,
            max_output_tokens: 900,
            temperature: 0.0,
            request_timeout_seconds: 5,
            recent_matches: 10,
            cache_ttl_minutes: 60,
            // On by default in tests, so the cached path is the one being
            // exercised everywhere rather than a path nothing covers.
            cache_enabled: true,
            chat_daily_limit: 50,
        },
        training: TrainingConfig {
            focus_weights: FocusWeights::default(),
            history_limit: 10,
        },
        // The production defaults: the confidence meter is Valve's model, and
        // a test that quietly calibrated at a different threshold would be
        // asserting against a rank nobody ships.
        calibration: CalibrationConfig {
            confidence_per_match_pct: 1.5,
            confidence_threshold_pct: 30.0,
            decay_days: 180,
            win_base_mmr: 30.0,
            loss_base_mmr: 25.0,
        },
        heroes: HeroConfig {
            meta_ttl_hours: 24,
            recommendation_limit: 8,
            benchmark_lookups: 5,
            fit_weights: FitWeights::default(),
            meta_weights: MetaWeights::default(),
        },
        roles: RoleConfig {
            // The production default. It used to be smaller, until the window
            // started deciding how much history the player model reads — at
            // which point a harness that quietly analysed a fifth of a test's
            // matches was testing something nobody ships.
            analysis_match_limit: 100,
            score_weights: RoleScoreWeights::default(),
        },
        llm: LlmConfig {
            base_url: "https://llm.example/v1".into(),
            api_key: None,
            model: "test".into(),
        },
        billing: BillingConfig {
            plan: "pro".into(),
            price_cents: 100,
            currency: "usd".into(),
            trial_days: 14,
            period_days: 30,
            history_limit: 20,
            sweep_interval_seconds: 3600,
            base_url: "https://pay.example/v1".into(),
            api_key: Some("test-key".into()),
            ipn_secret: Some("test-secret".into()),
            pay_currency: None,
            request_timeout_seconds: 5,
            // On by default: the gate is the behaviour under test, and a
            // harness that silently disabled it would make every paywall
            // assertion vacuous.
            enforce: true,
        },
        // Mounted in the harness so the routing itself is exercised; the tests
        // that care about the gate build their own config either way.
        docs_enabled: true,
    }
}

/// Connect to the test database, or `None` when one is not configured.
///
/// Ownership, uniqueness and pagination are enforced in SQL, so these tests
/// need a real Postgres. They are skipped loudly rather than silently passing.
pub async fn pool() -> Option<PgPool> {
    // Server-side errors are logged rather than returned, so a failing test is
    // mute without a subscriber. `try_init` because every test calls this.
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()?;

    match PgPool::connect(&url).await {
        Ok(pool) => {
            sqlx::migrate!("./migrations")
                .run(&pool)
                .await
                .expect("migrations must apply to the test database");
            Some(pool)
        }
        Err(e) => panic!("DATABASE_URL is set but unreachable: {e}"),
    }
}

/// Print the reason a test did nothing, so a skipped run is never mistaken for
/// a passing one.
pub fn skip(test: &str) {
    eprintln!("SKIPPED {test}: set DATABASE_URL or TEST_DATABASE_URL to run it");
}

pub fn app(db: PgPool, dota: Arc<MockDota>, verifier: Arc<dyn SteamVerifier>) -> TestApp {
    app_with(db, dota, verifier, StubBenchmarks::serving(), test_config())
}

/// The default harness with one specific coaching model.
pub fn app_with_llm(
    db: PgPool,
    dota: Arc<MockDota>,
    verifier: Arc<dyn SteamVerifier>,
    llm: Arc<dyn LlmProvider>,
) -> TestApp {
    app_with_providers(
        db,
        dota,
        verifier,
        StubBenchmarks::serving(),
        StubHeroMeta::serving(),
        llm,
        test_config(),
    )
}

/// Control over every stubbed dependency except hero meta, which serves.
pub fn app_with(
    db: PgPool,
    dota: Arc<MockDota>,
    verifier: Arc<dyn SteamVerifier>,
    benchmarks: Arc<dyn BenchmarkProvider>,
    config: Config,
) -> TestApp {
    app_with_providers(
        db,
        dota,
        verifier,
        benchmarks,
        StubHeroMeta::serving(),
        StubLlm::answering(),
        config,
    )
}

/// Full control over every stubbed dependency except payments, which take
/// money without complaint.
pub fn app_with_providers(
    db: PgPool,
    dota: Arc<MockDota>,
    verifier: Arc<dyn SteamVerifier>,
    benchmarks: Arc<dyn BenchmarkProvider>,
    hero_meta: Arc<dyn HeroMetaProvider>,
    llm: Arc<dyn LlmProvider>,
    config: Config,
) -> TestApp {
    build(
        db,
        dota,
        verifier,
        benchmarks,
        hero_meta,
        llm,
        StubPayments::taking_payments(),
        config,
    )
}

/// The default harness with one specific payment provider.
pub fn app_with_payments(
    db: PgPool,
    dota: Arc<MockDota>,
    verifier: Arc<dyn SteamVerifier>,
    payments: Arc<dyn PaymentProvider>,
    config: Config,
) -> TestApp {
    build(
        db,
        dota,
        verifier,
        StubBenchmarks::serving(),
        StubHeroMeta::serving(),
        StubLlm::answering(),
        payments,
        config,
    )
}

#[allow(clippy::too_many_arguments)]
fn build(
    db: PgPool,
    dota: Arc<MockDota>,
    verifier: Arc<dyn SteamVerifier>,
    benchmarks: Arc<dyn BenchmarkProvider>,
    hero_meta: Arc<dyn HeroMetaProvider>,
    llm: Arc<dyn LlmProvider>,
    payments: Arc<dyn PaymentProvider>,
    config: Config,
) -> TestApp {
    let steam = Arc::new(SteamOpenId::new(reqwest::Client::new(), &config.auth));

    let state = AppState::new(
        db.clone(),
        config.clone(),
        Providers {
            dota,
            steam,
            steam_verifier: verifier,
            benchmarks,
            hero_meta,
            llm,
            payments,
        },
    );

    TestApp {
        router: api::routes::build(state, &config),
        db,
    }
}

pub fn app_with_config(
    db: PgPool,
    dota: Arc<MockDota>,
    verifier: Arc<dyn SteamVerifier>,
    config: Config,
) -> TestApp {
    app_with(db, dota, verifier, StubBenchmarks::serving(), config)
}

impl TestApp {
    pub async fn request(&self, request: Request<Body>) -> TestResponse {
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("router should not fail");

        let status = response.status();
        let cookies: Vec<String> = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok().map(str::to_string))
            .collect();
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);

        let headers: HashMap<String, String> = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                Some((name.as_str().to_string(), value.to_str().ok()?.to_string()))
            })
            .collect();

        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8_lossy(&bytes).to_string();

        TestResponse {
            status,
            cookies,
            location,
            headers,
            body,
        }
    }

    pub async fn get(&self, path: &str, session: Option<&str>) -> TestResponse {
        self.request(build_request("GET", path, session)).await
    }

    pub async fn post(&self, path: &str, session: Option<&str>) -> TestResponse {
        self.request(build_request("POST", path, session)).await
    }

    /// Choose a coaching role.
    ///
    /// Every coaching read requires one: coaching without a chosen role would
    /// mean the server picking a role for the player, which is the decision the
    /// whole flow exists to leave with them. Tests that exercise coaching
    /// therefore make the choice first, exactly as the UI does.
    pub async fn choose_role(&self, session: &Session, role: &str) {
        let response = self
            .post_json(
                "/api/coach/role",
                &format!(r#"{{"role":"{role}"}}"#),
                Some(&session.token),
            )
            .await;

        assert_eq!(
            response.status,
            StatusCode::OK,
            "choosing a coaching role failed: {}",
            response.body
        );
    }

    /// POST a JSON body as a signed-in user.
    pub async fn post_json(&self, path: &str, body: &str, session: Option<&str>) -> TestResponse {
        let mut builder = Request::builder()
            .method("POST")
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json");

        if let Some(token) = session {
            builder = builder.header(
                header::COOKIE,
                format!(
                    "{}={token}",
                    dota_coach_backend::domain::session::SESSION_COOKIE
                ),
            );
        }

        self.request(builder.body(Body::from(body.to_string())).unwrap())
            .await
    }

    /// Create a signed-in user directly, bypassing the OpenID round trip.
    ///
    /// The login flow itself is covered separately; every other test only
    /// needs a valid session.
    pub async fn login_as(&self, steam_id: i64) -> Session {
        let user_id: Uuid = sqlx::query_scalar(
            "INSERT INTO users (steam_id, last_login_at) VALUES ($1, now())
             ON CONFLICT (steam_id) DO UPDATE SET last_login_at = now()
             RETURNING id",
        )
        .bind(steam_id)
        .fetch_one(&self.db)
        .await
        .unwrap();

        let dota_player_id: Uuid = sqlx::query_scalar(
            "INSERT INTO dota_players (user_id, steam_id, dota_account_id)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id) DO UPDATE SET user_id = EXCLUDED.user_id
             RETURNING id",
        )
        .bind(user_id)
        .bind(steam_id)
        .bind(steam_id - STEAM_ID64_BASE)
        .fetch_one(&self.db)
        .await
        .unwrap();

        let token = NewToken::generate();
        sqlx::query("INSERT INTO sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
            .bind(user_id)
            .bind(&token.hash)
            .bind(Utc::now() + Duration::hours(1))
            .execute(&self.db)
            .await
            .unwrap();

        Session {
            dota_player_id,
            token: token.plaintext,
        }
    }

    /// POST a body with arbitrary headers — what a provider notification is.
    pub async fn post_body(
        &self,
        path: &str,
        body: &str,
        headers: &[(&str, &str)],
    ) -> TestResponse {
        let mut builder = Request::builder()
            .method("POST")
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json");

        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }

        self.request(builder.body(Body::from(body.to_string())).unwrap())
            .await
    }

    /// Age an account's trial out, the way fourteen days would.
    pub async fn expire_trial(&self, steam_id: i64) {
        sqlx::query(
            "UPDATE subscriptions
                SET trial_started_at = now() - interval '30 days',
                    trial_ends_at    = now() - interval '16 days'
              WHERE user_id = (SELECT id FROM users WHERE steam_id = $1)",
        )
        .bind(steam_id)
        .execute(&self.db)
        .await
        .unwrap();
    }

    /// What the database says about an account's subscription, for assertions
    /// that must not go through the API that wrote it.
    pub async fn stored_subscription(
        &self,
        steam_id: i64,
    ) -> Option<(String, Option<DateTime<Utc>>)> {
        sqlx::query_as(
            "SELECT s.status, s.current_period_end
               FROM subscriptions s
               JOIN users u ON u.id = s.user_id
              WHERE u.steam_id = $1",
        )
        .bind(steam_id)
        .fetch_optional(&self.db)
        .await
        .unwrap()
    }

    /// Insert a session that already expired.
    pub async fn expired_session_for(&self, steam_id: i64) -> String {
        let session = self.login_as(steam_id).await;

        sqlx::query(
            "UPDATE sessions SET expires_at = now() - interval '1 hour' WHERE token_hash = $1",
        )
        .bind(hash_token(&session.token))
        .execute(&self.db)
        .await
        .unwrap();

        session.token
    }

    pub async fn stored_match_ids(&self, dota_player_id: Uuid) -> Vec<i64> {
        sqlx::query_scalar(
            "SELECT match_id FROM matches WHERE dota_player_id = $1 ORDER BY match_id",
        )
        .bind(dota_player_id)
        .fetch_all(&self.db)
        .await
        .unwrap()
    }

    /// A session for an account that completed Steam login but has no Dota
    /// identity linked — the state every `DOTA_ACCOUNT_NOT_LINKED` path is
    /// about, and one `login_as` cannot produce because it always links.
    pub async fn login_without_dota_link(&self, steam_id: i64) -> String {
        let user_id: Uuid = sqlx::query_scalar(
            "INSERT INTO users (steam_id, last_login_at) VALUES ($1, now())
             ON CONFLICT (steam_id) DO UPDATE SET last_login_at = now()
             RETURNING id",
        )
        .bind(steam_id)
        .fetch_one(&self.db)
        .await
        .unwrap();

        let token = NewToken::generate();
        sqlx::query("INSERT INTO sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
            .bind(user_id)
            .bind(&token.hash)
            .bind(Utc::now() + Duration::hours(1))
            .execute(&self.db)
            .await
            .unwrap();

        token.plaintext
    }

    /// Backdate a rank reading, so a test can have the two anchors a modeled
    /// trajectory segment needs. Sync only ever writes today's.
    pub async fn insert_rank_snapshot(
        &self,
        dota_account_id: i64,
        rank_tier: Option<i16>,
        at: DateTime<Utc>,
    ) {
        sqlx::query(
            "INSERT INTO rank_snapshots (dota_account_id, rank_tier, captured_at)
             VALUES ($1, $2, $3)",
        )
        .bind(dota_account_id)
        .bind(rank_tier)
        .bind(at)
        .execute(&self.db)
        .await
        .unwrap();
    }

    /// This account's rank readings, oldest first.
    ///
    /// Keyed by `dota_account_id` rather than the `dota_players.id` UUID the
    /// other helpers take, because that is how `rank_snapshots` is keyed —
    /// the assertion has to go through the same key the write did.
    pub async fn stored_rank_snapshots(
        &self,
        dota_account_id: i64,
    ) -> Vec<(Option<i16>, Option<i32>)> {
        sqlx::query_as(
            "SELECT rank_tier, leaderboard_rank
               FROM rank_snapshots
              WHERE dota_account_id = $1
              ORDER BY captured_at ASC",
        )
        .bind(dota_account_id)
        .fetch_all(&self.db)
        .await
        .unwrap()
    }

    /// Remove everything a test created. Tests share a database, so each one
    /// cleans up after itself rather than truncating shared tables.
    pub async fn cleanup(&self, steam_ids: &[i64]) {
        sqlx::query("DELETE FROM users WHERE steam_id = ANY($1)")
            .bind(steam_ids)
            .execute(&self.db)
            .await
            .unwrap();
    }
}

pub struct Session {
    pub dota_player_id: Uuid,
    pub token: String,
}

pub struct TestResponse {
    pub status: StatusCode,
    pub cookies: Vec<String>,
    pub location: Option<String>,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl TestResponse {
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("expected JSON, got {:?}: {e}", self.body))
    }

    pub fn error_code(&self) -> String {
        self.json()["error"]["code"]
            .as_str()
            .unwrap_or("")
            .to_string()
    }

    /// A response header, lowercased as HTTP/2 and Axum store them.
    pub fn header(&self, name: &str) -> Option<String> {
        self.headers.get(&name.to_lowercase()).cloned()
    }

    pub fn cookie_value(&self, name: &str) -> Option<String> {
        self.cookies.iter().find_map(|raw| {
            let pair = raw.split(';').next()?;
            let (key, value) = pair.split_once('=')?;
            (key.trim() == name).then(|| value.trim().to_string())
        })
    }
}

fn build_request(method: &str, path: &str, session: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(path);

    if let Some(token) = session {
        builder = builder.header(
            header::COOKIE,
            format!(
                "{}={token}",
                dota_coach_backend::domain::session::SESSION_COOKIE
            ),
        );
    }

    builder.body(Body::empty()).unwrap()
}

/// A unique Steam id per test, so tests sharing a database never collide.
pub fn unique_steam_id() -> i64 {
    // Account ids must stay inside 32 bits.
    STEAM_ID64_BASE + (rand::random::<u32>() as i64 % 2_000_000_000) + 1
}

pub fn sample_match(match_id: i64, started_at: DateTime<Utc>) -> NormalizedMatch {
    NormalizedMatch {
        match_id,
        hero_id: 35,
        won: match_id % 2 == 0,
        duration_seconds: 2_400,
        kills: 8,
        deaths: 4,
        assists: 12,
        gpm: 550,
        xpm: 620,
        last_hits: 300,
        denies: None,
        net_worth: None,
        hero_damage: Some(25_000),
        tower_damage: Some(3_000),
        hero_healing: Some(0),
        lane_role: Some(1),
        is_roaming: Some(false),
        farm_rank: None,
        game_mode: Some(22),
        lobby_type: Some(7),
        party_size: Some(1),
        started_at,
        from_details: false,
        team_kills: Some(30),
        team_deaths: Some(20),
        replay_parsed: false,
        last_hits_at_10: None,
        last_hits_at_15: None,
        gold_at_10: None,
        gold_at_15: None,
        xp_at_10: None,
        xp_at_15: None,
        bkb_seconds: None,
        blink_seconds: None,
        midas_seconds: None,
        teamfight_participation: None,
    }
}

/// Matches with a controllable id range and death count.
///
/// Pattern detection is about rates across a history, so a test needs to be
/// able to append a *second* batch that does not collide with the first.
/// Matches played over the last `count` days, newest last.
///
/// `sample_matches` is pinned to a fixed 2023 timestamp, which is fine for
/// anything that only cares about ordering and stays wrong for anything with a
/// window: by now those games are years old and fall outside every recency
/// bound in the product.
pub fn recent_matches(count: i64) -> Vec<NormalizedMatch> {
    let now = Utc::now();
    (0..count)
        .map(|i| {
            let started = now - Duration::days(count - i);
            sample_match(9_500_000_000 + i, started)
        })
        .collect()
}

pub fn matches_with(count: i64, start_id: i64, deaths: i32) -> Vec<NormalizedMatch> {
    (0..count)
        .map(|i| {
            let started = Utc
                .timestamp_opt(1_700_000_000 + (start_id + i) * 3_600, 0)
                .single()
                .unwrap();
            let mut m = sample_match(start_id + i, started);
            m.deaths = deaths;
            m
        })
        .collect()
}

pub fn sample_matches(count: i64) -> Vec<NormalizedMatch> {
    (0..count)
        .map(|i| {
            let started = Utc
                .timestamp_opt(1_700_000_000 + i * 3_600, 0)
                .single()
                .unwrap();
            sample_match(9_000_000_000 + i, started)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Competitive-population fixtures
// ---------------------------------------------------------------------------
//
// Shared by every test binary that needs a player whose history is a known mix
// of game modes and roles. Kept here rather than in one test file so a second
// binary does not end up with a second, subtly different definition of "a
// Turbo carry game".

/// Ranked All Pick, as OpenDota reports it: `all_draft` in a ranked lobby.
pub const RANKED_ALL_PICK: (i32, i32) = (22, 7);
/// Unranked public All Pick.
pub const PUBLIC_ALL_PICK: (i32, i32) = (22, 0);
pub const TURBO: (i32, i32) = (23, 0);
pub const ABILITY_DRAFT: (i32, i32) = (18, 7);
/// All Pick, but in a tournament lobby: the right draft, the wrong population.
pub const TOURNAMENT: (i32, i32) = (22, 2);

/// How a seeded match should be read by the role estimator.
#[derive(Clone, Copy)]
pub enum Lane {
    Carry,
    Mid,
    Offlane,
    Support,
    HardSupport,
    /// An unparsed replay: farm priority says core, nothing says which lane.
    UnclassifiedCore,
}

/// One batch of matches with a fixed mode and role.
///
/// Ids are offset per batch so several batches can be seeded into one player's
/// history without colliding. `wins` counts from the start of the batch.
pub fn batch(
    start_id: i64,
    count: i64,
    mode: (i32, i32),
    lane: Lane,
    wins: i64,
) -> Vec<NormalizedMatch> {
    (0..count)
        .map(|i| {
            let started = Utc
                .timestamp_opt(1_700_000_000 + (start_id + i) * 3_600, 0)
                .single()
                .unwrap();

            let mut m = sample_match(start_id + i, started);
            m.game_mode = Some(mode.0);
            m.lobby_type = Some(mode.1);
            m.won = i < wins;

            match lane {
                Lane::Carry => {
                    m.lane_role = Some(1);
                    m.last_hits = 300;
                }
                Lane::Mid => {
                    m.lane_role = Some(2);
                    m.last_hits = 280;
                }
                Lane::Offlane => {
                    m.lane_role = Some(3);
                    m.last_hits = 220;
                }
                Lane::Support => {
                    m.lane_role = Some(3);
                    m.last_hits = 40;
                }
                Lane::HardSupport => {
                    m.lane_role = Some(1);
                    m.last_hits = 30;
                }
                Lane::UnclassifiedCore => {
                    m.lane_role = None;
                    m.farm_rank = Some(1);
                    m.last_hits = 250;
                }
            }

            m
        })
        .collect()
}

/// Sync a crafted history and keep the router, so a test can go through the
/// HTTP surface the product actually serves.
///
/// `window` is the competitive window the app is configured with — what
/// `/api/stats`, `/api/coach/roles` and `?scope=competitive` read.
pub async fn seed_app(
    db: PgPool,
    matches: Vec<NormalizedMatch>,
    window: i64,
) -> (TestApp, Session) {
    let mut config = test_config();
    // Above anything these tests seed: the sync cap is a separate concern with
    // its own test, and letting it truncate here would make the window
    // assertions vacuous.
    config.dota.sync_match_limit = 500;
    config.roles.analysis_match_limit = window;

    let app = app_with_config(
        db,
        MockDota::with_matches(matches),
        StubVerifier::rejecting(),
        config,
    );

    let session = app.login_as(unique_steam_id()).await;
    let response = app.post("/api/players/me/sync", Some(&session.token)).await;
    assert_eq!(
        response.status,
        StatusCode::OK,
        "seeding sync failed: {}",
        response.body
    );

    (app, session)
}
