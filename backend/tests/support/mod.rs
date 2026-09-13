//! Test harness: the real router, with the two external dependencies stubbed.
//!
//! Nothing here reaches OpenDota or Valve. The database is real, because the
//! behaviour under test (uniqueness, ownership scoping, pagination) lives in
//! SQL and would be meaningless against a fake.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use chrono::{DateTime, Duration, TimeZone, Utc};
use dota_coach_backend::api;
use dota_coach_backend::config::{AuthConfig, Config, DotaConfig, LlmConfig};
use dota_coach_backend::domain::r#match::NormalizedMatch;
use dota_coach_backend::domain::session::{hash_token, NewToken};
use dota_coach_backend::services::auth::steam_openid::{OpenIdError, SteamOpenId, SteamVerifier};
use dota_coach_backend::services::dota::{DotaDataProvider, ProviderError, ProviderPlayer};
use dota_coach_backend::state::AppState;
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
        },
        llm: LlmConfig {
            base_url: "https://llm.example/v1".into(),
            api_key: None,
            model: "test".into(),
        },
    }
}

/// Connect to the test database, or `None` when one is not configured.
///
/// Ownership, uniqueness and pagination are enforced in SQL, so these tests
/// need a real Postgres. They are skipped loudly rather than silently passing.
pub async fn pool() -> Option<PgPool> {
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
    let config = test_config();
    let steam = Arc::new(SteamOpenId::new(reqwest::Client::new(), &config.auth));

    let state = AppState::new(db.clone(), config.clone(), dota.clone(), steam, verifier);

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
    let steam = Arc::new(SteamOpenId::new(reqwest::Client::new(), &config.auth));
    let state = AppState::new(db.clone(), config.clone(), dota, steam, verifier);

    TestApp {
        router: api::routes::build(state, &config),
        db,
    }
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

        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8_lossy(&bytes).to_string();

        TestResponse {
            status,
            cookies,
            location,
            body,
        }
    }

    pub async fn get(&self, path: &str, session: Option<&str>) -> TestResponse {
        self.request(build_request("GET", path, session)).await
    }

    pub async fn post(&self, path: &str, session: Option<&str>) -> TestResponse {
        self.request(build_request("POST", path, session)).await
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
    }
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
