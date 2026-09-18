use std::env;

use crate::domain::hero::FitWeights;
use crate::domain::role::RoleScoreWeights;
use crate::domain::training::FocusWeights;
use crate::services::hero_meta::strength::MetaWeights;

/// Application configuration, sourced exclusively from environment variables.
///
/// Secrets (`llm_api_key`, database credentials) live here and are never
/// serialized or sent to the frontend.
// Provider fields are parsed and validated at startup but only consumed once
// the Dota and LLM services land in Phase 2/4.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    /// Comma-separated list of origins allowed to call the API.
    pub cors_origins: Vec<String>,
    pub auth: AuthConfig,
    pub dota: DotaConfig,
    pub heroes: HeroConfig,
    pub roles: RoleConfig,
    pub coach: CoachConfig,
    pub training: TrainingConfig,
    pub llm: LlmConfig,
    pub billing: BillingConfig,
    /// Whether to mount Swagger UI and the OpenAPI document.
    ///
    /// Off by default anywhere that looks like production. The spec is not a
    /// secret — every route it describes still enforces its own session — but
    /// it is a complete map of the API's surface, and publishing one to the
    /// internet should be a decision somebody made rather than a default they
    /// inherited.
    pub docs_enabled: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct AuthConfig {
    /// Public origin of *this* API, used to build `openid.return_to`. Steam
    /// signs the value, so it must match what the browser actually reaches.
    pub public_base_url: String,
    /// Where the user lands after a completed or failed login.
    pub frontend_base_url: String,
    /// Steam's OpenID 2.0 endpoint. Overridable so tests never touch Valve.
    pub steam_openid_url: String,
    pub session_ttl_hours: i64,
    /// Set the `Secure` flag on cookies. Must be true in production; false
    /// allows plain-HTTP local development.
    pub cookie_secure: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct DotaConfig {
    pub base_url: String,
    /// Optional OpenDota key. Raises the rate limit; the free tier works without it.
    pub api_key: Option<String>,
    /// How many recent matches a single sync pulls.
    pub sync_match_limit: u32,
    /// Minimum gap between two syncs of the same player, in seconds.
    pub sync_cooldown_seconds: i64,
    pub request_timeout_seconds: u64,
    /// How long a cached peer distribution stays fresh. These move slowly —
    /// a day-old distribution is still a fair comparison.
    pub benchmark_ttl_hours: i64,
    /// Restrict syncing to "significant" matches — OpenDota's term for the
    /// standard competitive modes, which excludes Turbo, Ability Draft and
    /// every event mode.
    ///
    /// Off by default. The provider applies this filter unless told otherwise,
    /// and for a player whose games are mostly Turbo that silently hides almost
    /// their entire history: the sync succeeds, reports nothing new, and gives
    /// no indication that anything was dropped. A coach that cannot see what
    /// you actually play is worse than one whose benchmarks need a caveat.
    pub significant_only: bool,
}

/// Hero Intelligence tuning.
///
/// The weights are configuration rather than code because the spec says so:
/// the initial numbers are a starting point to be revised once there is real
/// usage, and revising them must not mean a deploy-shaped change to the
/// scoring engine.
#[derive(Clone, Debug)]
pub struct HeroConfig {
    /// How long a cached meta cohort stays fresh. Hero meta moves over days,
    /// not minutes.
    pub meta_ttl_hours: i64,
    /// How many candidates the recommendation endpoints return by default.
    pub recommendation_limit: usize,
    /// Ceiling on peer-distribution fetches per request. Each one is a
    /// potential upstream call, so the benchmark component is only filled in
    /// for the player's most-played heroes.
    pub benchmark_lookups: usize,
    pub fit_weights: FitWeights,
    pub meta_weights: MetaWeights,
}

impl HeroConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let defaults = FitWeights::default();
        let meta_defaults = MetaWeights::default();

        let weights = FitWeights {
            player_performance: parsed("FIT_WEIGHT_PERFORMANCE", defaults.player_performance)?,
            meta_strength: parsed("FIT_WEIGHT_META", defaults.meta_strength)?,
            experience: parsed("FIT_WEIGHT_EXPERIENCE", defaults.experience)?,
            benchmark: parsed("FIT_WEIGHT_BENCHMARK", defaults.benchmark)?,
            recent_form: parsed("FIT_WEIGHT_RECENT_FORM", defaults.recent_form)?,
        };

        validate_weights(&weights)?;

        Ok(Self {
            meta_ttl_hours: parsed("HERO_META_TTL_HOURS", 24)?,
            recommendation_limit: parsed::<usize>("HERO_RECOMMENDATION_LIMIT", 8)?.clamp(1, 50),
            benchmark_lookups: parsed::<usize>("HERO_BENCHMARK_LOOKUPS", 5)?.clamp(0, 20),
            fit_weights: weights,
            meta_weights: meta_defaults,
        })
    }
}

/// AI coaching limits.
///
/// Every value here bounds something a user can trigger: how often a model
/// call happens, how long it may run, and how much it may say. An LLM call is
/// the only operation in this service that costs money per request, so it is
/// the only one with a per-user budget.
#[derive(Clone, Debug)]
pub struct CoachConfig {
    /// Minimum gap between two generations for the same player.
    pub cooldown_seconds: i64,
    /// Ceiling on generations per player per rolling 24 hours.
    pub daily_limit: i64,
    /// Ceiling on insights kept from one answer.
    pub max_insights: usize,
    /// Ceiling on training-plan steps kept from one answer. Small on purpose:
    /// a plan with ten items is a list, and a player works on one thing.
    pub max_plan_steps: usize,
    pub max_output_tokens: u32,
    /// Low: this is interpretation of fixed numbers, not creative writing.
    pub temperature: f32,
    /// How long a single model call may take before it is abandoned.
    pub request_timeout_seconds: u64,
    /// How many recent matches feed the evidence builder.
    pub recent_matches: i64,
}

impl CoachConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            cooldown_seconds: parsed("COACH_COOLDOWN_SECONDS", 30)?,
            daily_limit: parsed::<i64>("COACH_DAILY_LIMIT", 20)?.max(0),
            max_insights: parsed::<usize>("COACH_MAX_INSIGHTS", 5)?.clamp(1, 20),
            max_plan_steps: parsed::<usize>("COACH_MAX_PLAN_STEPS", 4)?.clamp(1, 10),
            // The ceiling has to cover a reasoning model, which spends the bulk
            // of this budget thinking and emits nothing at all if it runs out
            // mid-thought. Raising the ceiling costs nothing on a model that
            // answers directly: the budget is a limit, not an allocation.
            max_output_tokens: parsed::<u32>("COACH_MAX_OUTPUT_TOKENS", 900)?.clamp(200, 16_000),
            temperature: parsed::<f32>("COACH_TEMPERATURE", 0.2)?.clamp(0.0, 2.0),
            // A reasoning model spends most of its budget thinking before it
            // emits a character, so a single coaching call can legitimately run
            // for over a minute. The ceiling is generous for that reason; the
            // default stays short for providers that answer immediately.
            request_timeout_seconds: parsed::<u64>("LLM_TIMEOUT_SECONDS", 30)?.clamp(1, 180),
            recent_matches: parsed::<i64>("COACH_RECENT_MATCHES", 10)?.clamp(1, 50),
        })
    }
}

/// Reject a weighting that cannot mean anything.
///
/// A negative weight would invert a component's meaning; an all-zero set would
/// leave the score undefined. Both are configuration mistakes worth failing the
/// boot for rather than silently absorbing. Named weights are checked first, so
/// the message points at the variable that is wrong rather than at the sum it
/// spoiled.
fn validate_weights(weights: &FitWeights) -> Result<(), ConfigError> {
    let named = [
        ("FIT_WEIGHT_PERFORMANCE", weights.player_performance),
        ("FIT_WEIGHT_META", weights.meta_strength),
        ("FIT_WEIGHT_EXPERIENCE", weights.experience),
        ("FIT_WEIGHT_BENCHMARK", weights.benchmark),
        ("FIT_WEIGHT_RECENT_FORM", weights.recent_form),
    ];

    for (key, value) in named {
        if value < 0.0 || !value.is_finite() {
            return Err(ConfigError::Invalid(key, "must be zero or positive".into()));
        }
    }
    if named.iter().map(|(_, v)| v).sum::<f32>() <= 0.0 {
        return Err(ConfigError::Invalid(
            "FIT_WEIGHT_*",
            "fit weights must sum to more than zero".into(),
        ));
    }

    Ok(())
}

/// Role analysis tuning.
///
/// The competitive window and how a role's performance score is composed. As
/// with the fit weights, the numbers are a starting point: the scoring rule is
/// fixed in code and documented in `services::roles`, the emphasis is not.
#[derive(Clone, Debug)]
pub struct RoleConfig {
    /// How many eligible matches the overall and role analysis reads.
    ///
    /// Applied *after* eligibility filtering, never before — see
    /// `domain::scope::MatchScope`.
    pub analysis_match_limit: i64,
    pub score_weights: RoleScoreWeights,
}

impl RoleConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let defaults = RoleScoreWeights::default();

        let weights = RoleScoreWeights {
            win_rate: parsed("ROLE_WEIGHT_WIN_RATE", defaults.win_rate)?,
            kill_participation: parsed(
                "ROLE_WEIGHT_KILL_PARTICIPATION",
                defaults.kill_participation,
            )?,
            kda: parsed("ROLE_WEIGHT_KDA", defaults.kda)?,
            deaths: parsed("ROLE_WEIGHT_DEATHS", defaults.deaths)?,
        };

        validate_role_weights(&weights)?;

        Ok(Self {
            // A hundred is the product's target window. The floor is twenty
            // because a smaller window would make the per-role samples
            // meaningless; the ceiling keeps one player's analysis from
            // scanning an unbounded history.
            analysis_match_limit: parsed::<i64>("ROLE_ANALYSIS_MATCH_LIMIT", 100)?.clamp(20, 500),
            score_weights: weights,
        })
    }
}

fn validate_role_weights(weights: &RoleScoreWeights) -> Result<(), ConfigError> {
    let named = [
        ("ROLE_WEIGHT_WIN_RATE", weights.win_rate),
        ("ROLE_WEIGHT_KILL_PARTICIPATION", weights.kill_participation),
        ("ROLE_WEIGHT_KDA", weights.kda),
        ("ROLE_WEIGHT_DEATHS", weights.deaths),
    ];

    for (key, value) in named {
        if value < 0.0 || !value.is_finite() {
            return Err(ConfigError::Invalid(key, "must be zero or positive".into()));
        }
    }
    if named.iter().map(|(_, v)| v).sum::<f32>() <= 0.0 {
        return Err(ConfigError::Invalid(
            "ROLE_WEIGHT_*",
            "role score weights must sum to more than zero".into(),
        ));
    }

    Ok(())
}

/// Training focus tuning.
///
/// The inputs are the spec's (§28); only their relative weight is
/// configuration. As with the fit weights, these are a starting point to be
/// revised once there is real usage.
#[derive(Clone, Debug)]
pub struct TrainingConfig {
    pub focus_weights: FocusWeights,
    /// How many past focuses the history endpoint returns.
    pub history_limit: i64,
}

impl TrainingConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let defaults = FocusWeights::default();

        let weights = FocusWeights {
            gap: parsed("FOCUS_WEIGHT_GAP", defaults.gap)?,
            pattern: parsed("FOCUS_WEIGHT_PATTERN", defaults.pattern)?,
            recent: parsed("FOCUS_WEIGHT_RECENT", defaults.recent)?,
            impact: parsed("FOCUS_WEIGHT_IMPACT", defaults.impact)?,
            confidence: parsed("FOCUS_WEIGHT_CONFIDENCE", defaults.confidence)?,
            recency: parsed("FOCUS_WEIGHT_RECENCY", defaults.recency)?,
        };
        validate_focus_weights(&weights)?;

        Ok(Self {
            focus_weights: weights,
            history_limit: parsed::<i64>("FOCUS_HISTORY_LIMIT", 10)?.clamp(1, 100),
        })
    }
}

/// Same rule as the fit weights: a negative weight inverts a component's
/// meaning, and an all-zero set leaves the score undefined.
fn validate_focus_weights(weights: &FocusWeights) -> Result<(), ConfigError> {
    let named = [
        ("FOCUS_WEIGHT_GAP", weights.gap),
        ("FOCUS_WEIGHT_PATTERN", weights.pattern),
        ("FOCUS_WEIGHT_RECENT", weights.recent),
        ("FOCUS_WEIGHT_IMPACT", weights.impact),
        ("FOCUS_WEIGHT_CONFIDENCE", weights.confidence),
        ("FOCUS_WEIGHT_RECENCY", weights.recency),
    ];

    for (key, value) in named {
        if value < 0.0 || !value.is_finite() {
            return Err(ConfigError::Invalid(key, "must be zero or positive".into()));
        }
    }
    if weights.total() <= 0.0 {
        return Err(ConfigError::Invalid(
            "FOCUS_WEIGHT_*",
            "focus weights must sum to more than zero".into(),
        ));
    }

    Ok(())
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
}

impl LlmConfig {
    /// Whether the LLM provider has enough configuration to be usable.
    /// Phase 1 only reports this at startup; later phases gate analysis on it.
    pub fn is_configured(&self) -> bool {
        self.api_key.as_ref().is_some_and(|k| !k.is_empty())
    }
}

/// Trial, price and payment provider.
///
/// The price is configuration in the strongest sense: `$9.98/month` is a launch
/// decision, not an invariant, and it appears exactly once in the system — in
/// `BILLING_PRICE_CENTS`. Minor units, because a float price is a rounding bug
/// waiting for a currency with different conventions.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct BillingConfig {
    pub plan: String,
    pub price_cents: i64,
    /// ISO-4217, lowercase. What the *price* is denominated in; the coin the
    /// user pays with is a separate, provider-side choice.
    pub currency: String,
    pub trial_days: i64,
    /// Length of one paid period.
    pub period_days: i64,
    /// How many charges the payments endpoint returns.
    pub history_limit: i64,
    /// How often the background sweep rewrites subscriptions whose window has
    /// closed. Trial/period boundaries are day-granularity, so an hourly sweep
    /// is far more precise than anything that matters downstream of it.
    pub sweep_interval_seconds: u64,

    pub base_url: String,
    pub api_key: Option<String>,
    /// Verifies notifications. Never used to sign an outgoing call.
    pub ipn_secret: Option<String>,
    /// Restrict checkout to one coin. `None` lets the user choose.
    pub pay_currency: Option<String>,
    pub request_timeout_seconds: u64,

    /// Whether an expired account actually loses premium features.
    ///
    /// Defaults to *off* when no payment provider is configured: a deployment
    /// that cannot take money has no business telling anyone to pay, and
    /// locking users out with no checkout to reach is worse than giving the
    /// feature away. Set `BILLING_ENFORCE=true` to enforce anyway.
    pub enforce: bool,
}

impl BillingConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let api_key = env::var("NOWPAYMENTS_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());
        let ipn_secret = env::var("NOWPAYMENTS_IPN_SECRET")
            .ok()
            .filter(|s| !s.is_empty());
        let configured = api_key.is_some() && ipn_secret.is_some();

        let price_cents = checked_price(parsed("BILLING_PRICE_CENTS", 998)?)?;

        Ok(Self {
            plan: optional("BILLING_PLAN", "pro"),
            price_cents,
            currency: optional("BILLING_CURRENCY", "usd").to_lowercase(),
            trial_days: parsed::<i64>("BILLING_TRIAL_DAYS", 14)?.clamp(0, 365),
            period_days: parsed::<i64>("BILLING_PERIOD_DAYS", 30)?.clamp(1, 366),
            history_limit: parsed::<i64>("BILLING_HISTORY_LIMIT", 20)?.clamp(1, 100),
            sweep_interval_seconds: parsed::<u64>("BILLING_SWEEP_INTERVAL_SECONDS", 3600)?
                .clamp(60, 86_400),
            base_url: optional("NOWPAYMENTS_BASE_URL", "https://api.nowpayments.io/v1"),
            api_key,
            ipn_secret,
            pay_currency: env::var("NOWPAYMENTS_PAY_CURRENCY")
                .ok()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_lowercase()),
            request_timeout_seconds: parsed::<u64>("BILLING_TIMEOUT_SECONDS", 15)?.clamp(1, 120),
            enforce: parsed("BILLING_ENFORCE", configured)?,
        })
    }

    /// Whether checkout can be offered at all on this deployment.
    pub fn is_configured(&self) -> bool {
        self.api_key.as_ref().is_some_and(|k| !k.is_empty())
            && self.ipn_secret.as_ref().is_some_and(|s| !s.is_empty())
    }
}

/// A subscription that costs nothing is a configuration mistake, not a
/// generous offer: the trial is how the product is given away, and a zero or
/// negative price would open charges no provider will accept.
fn checked_price(cents: i64) -> Result<i64, ConfigError> {
    if cents <= 0 {
        return Err(ConfigError::Invalid(
            "BILLING_PRICE_CENTS",
            "must be greater than zero".into(),
        ));
    }

    Ok(cents)
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    Missing(&'static str),
    #[error("invalid value for environment variable {0}: {1}")]
    Invalid(&'static str, String),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        // Lifted out of the literal below because the docs default is derived
        // from it: whether this deployment looks like production is a question
        // only the auth settings can answer.
        let auth = AuthConfig {
            public_base_url: trim_trailing_slash(&optional(
                "PUBLIC_BASE_URL",
                "http://localhost:8080",
            )),
            frontend_base_url: trim_trailing_slash(&optional(
                "FRONTEND_BASE_URL",
                "http://localhost:3000",
            )),
            steam_openid_url: optional(
                "STEAM_OPENID_URL",
                "https://steamcommunity.com/openid/login",
            ),
            session_ttl_hours: parsed("SESSION_TTL_HOURS", 720)?,
            cookie_secure: parsed("COOKIE_SECURE", false)?,
        };

        Ok(Self {
            docs_enabled: docs_enabled(&auth)?,
            database_url: required("DATABASE_URL")?,
            host: optional("HOST", "0.0.0.0"),
            port: optional("PORT", "8080")
                .parse()
                .map_err(|e| ConfigError::Invalid("PORT", format!("{e}")))?,
            cors_origins: optional("CORS_ORIGINS", "http://localhost:3000")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            auth,
            dota: DotaConfig {
                base_url: optional("DOTA_API_BASE_URL", "https://api.opendota.com/api"),
                api_key: env::var("DOTA_API_KEY").ok().filter(|s| !s.is_empty()),
                // The ceiling is what decides whether the hundred-match
                // competitive window is reachable at all: the limit counts
                // *raw* matches, and a player whose history is half Turbo needs
                // to pull two hundred to store a hundred eligible ones. The
                // default stays low because a first sync fetches a match detail
                // per new match, and a deep first pull can outlast the route's
                // own timeout — raising it is an operator's decision.
                sync_match_limit: parsed("SYNC_MATCH_LIMIT", 20)?.clamp(1, 500),
                sync_cooldown_seconds: parsed("SYNC_COOLDOWN_SECONDS", 30)?,
                request_timeout_seconds: parsed("DOTA_API_TIMEOUT_SECONDS", 10)?,
                benchmark_ttl_hours: parsed("BENCHMARK_TTL_HOURS", 24)?,
                significant_only: parsed("DOTA_SIGNIFICANT_ONLY", false)?,
            },
            heroes: HeroConfig::from_env()?,
            roles: RoleConfig::from_env()?,
            coach: CoachConfig::from_env()?,
            training: TrainingConfig::from_env()?,
            llm: LlmConfig {
                base_url: optional("LLM_BASE_URL", "https://api.openai.com/v1"),
                api_key: env::var("LLM_API_KEY").ok().filter(|s| !s.is_empty()),
                model: optional("LLM_MODEL", "gpt-4o-mini"),
            },
            billing: BillingConfig::from_env()?,
        })
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Whether to serve the API documentation.
///
/// `DOCS_ENABLED` decides it outright when set. Unset, it follows the same
/// signal the boot-time deployment warnings use: a deployment reachable at
/// localhost is somebody's machine and gets the docs; anything else is treated
/// as production and does not, until an operator says otherwise.
///
/// The failure modes are deliberately asymmetric. Docs missing in development
/// is an inconvenience discovered in seconds. Docs published from production by
/// accident is a full map of the API handed to anyone who guesses `/docs`, and
/// nobody finds out at all.
fn docs_enabled(auth: &AuthConfig) -> Result<bool, ConfigError> {
    if let Ok(raw) = env::var("DOCS_ENABLED") {
        if !raw.trim().is_empty() {
            return raw.trim().parse::<bool>().map_err(|_| {
                ConfigError::Invalid("DOCS_ENABLED", "expected `true` or `false`".into())
            });
        }
    }

    let local = auth.public_base_url.starts_with("http://localhost")
        || auth.public_base_url.starts_with("http://127.");

    Ok(local && !auth.cookie_secure)
}

fn required(key: &'static str) -> Result<String, ConfigError> {
    env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .ok_or(ConfigError::Missing(key))
}

/// Parse an optional numeric variable, falling back to `default` when unset.
fn parsed<T>(key: &'static str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key).ok().filter(|v| !v.is_empty()) {
        None => Ok(default),
        Some(raw) => raw
            .trim()
            .parse()
            .map_err(|e| ConfigError::Invalid(key, format!("{e}"))),
    }
}

/// URLs are concatenated with paths throughout, so store them without a
/// trailing slash and never produce `//` in a redirect.
fn trim_trailing_slash(value: &str) -> String {
    value.trim_end_matches('/').to_string()
}

fn optional(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn llm(key: Option<&str>) -> LlmConfig {
        LlmConfig {
            base_url: "https://example.test/v1".into(),
            api_key: key.map(str::to_string),
            model: "test-model".into(),
        }
    }

    #[test]
    fn llm_is_configured_only_with_a_non_empty_key() {
        assert!(!llm(None).is_configured());
        assert!(!llm(Some("")).is_configured());
        assert!(llm(Some("sk-test")).is_configured());
    }

    fn auth_at(base_url: &str, cookie_secure: bool) -> AuthConfig {
        AuthConfig {
            public_base_url: base_url.into(),
            frontend_base_url: "http://localhost:3000".into(),
            steam_openid_url: "https://steamcommunity.com/openid/login".into(),
            session_ttl_hours: 720,
            cookie_secure,
        }
    }

    /// One test rather than four, because `DOCS_ENABLED` is process-global and
    /// parallel tests that set and clear it race each other. The cases are
    /// ordered: explicit values first, then the fallback with the variable
    /// unset.
    #[test]
    fn the_docs_flag_is_explicit_first_and_production_safe_by_default() {
        let production = auth_at("https://api.dota-coach.example", true);
        let development = auth_at("http://localhost:8080", false);

        env::set_var("DOCS_ENABLED", "true");
        assert!(
            docs_enabled(&production).unwrap(),
            "an explicit opt-in wins over the heuristic"
        );

        env::set_var("DOCS_ENABLED", "false");
        assert!(
            !docs_enabled(&development).unwrap(),
            "an explicit opt-out wins too"
        );

        // A typo would otherwise resolve to "production", silently, and whoever
        // wrote `DOCS_ENABLED=yes` would never learn why the docs vanished.
        env::set_var("DOCS_ENABLED", "yes");
        assert!(matches!(
            docs_enabled(&development),
            Err(ConfigError::Invalid("DOCS_ENABLED", _))
        ));

        // Unset: the default is the whole point of the flag. An operator who
        // never sets it must not publish the API's map from production.
        env::remove_var("DOCS_ENABLED");
        assert!(
            docs_enabled(&development).unwrap(),
            "a localhost deployment is somebody's machine"
        );
        assert!(docs_enabled(&auth_at("http://127.0.0.1:8080", false)).unwrap());
        assert!(
            !docs_enabled(&production).unwrap(),
            "a public origin is production until told otherwise"
        );
        assert!(
            !docs_enabled(&auth_at("http://localhost:8080", true)).unwrap(),
            "secure cookies on localhost still reads as a production config"
        );
    }

    #[test]
    fn bind_address_joins_host_and_port() {
        let config = Config {
            docs_enabled: true,
            database_url: "postgres://localhost/test".into(),
            host: "127.0.0.1".into(),
            port: 9000,
            cors_origins: vec![],
            auth: AuthConfig {
                public_base_url: "http://localhost:8080".into(),
                frontend_base_url: "http://localhost:3000".into(),
                steam_openid_url: "https://steamcommunity.com/openid/login".into(),
                session_ttl_hours: 720,
                cookie_secure: false,
            },
            dota: DotaConfig {
                base_url: "https://api.opendota.com/api".into(),
                api_key: None,
                sync_match_limit: 20,
                sync_cooldown_seconds: 30,
                request_timeout_seconds: 10,
                benchmark_ttl_hours: 24,
                significant_only: false,
            },
            heroes: HeroConfig::from_env().unwrap(),
            roles: RoleConfig::from_env().unwrap(),
            coach: CoachConfig::from_env().unwrap(),
            training: TrainingConfig::from_env().unwrap(),
            llm: llm(None),
            billing: BillingConfig::from_env().unwrap(),
        };
        assert_eq!(config.bind_address(), "127.0.0.1:9000");
    }

    #[test]
    fn billing_defaults_to_the_launch_offer() {
        let billing = BillingConfig::from_env().unwrap();

        assert_eq!(billing.price_cents, 998, "$9.98");
        assert_eq!(billing.currency, "usd");
        assert_eq!(billing.trial_days, 14);
        assert_eq!(billing.period_days, 30);
    }

    #[test]
    fn a_free_price_is_a_configuration_mistake_worth_failing_the_boot_for() {
        assert_eq!(checked_price(100).unwrap(), 100);
        assert!(matches!(
            checked_price(0),
            Err(ConfigError::Invalid("BILLING_PRICE_CENTS", _))
        ));
        assert!(matches!(
            checked_price(-1),
            Err(ConfigError::Invalid("BILLING_PRICE_CENTS", _))
        ));
    }

    #[test]
    fn a_deployment_that_cannot_take_money_does_not_lock_anyone_out() {
        // No provider credentials in the test environment, so enforcement is
        // off by default and checkout reports itself unavailable.
        let billing = BillingConfig::from_env().unwrap();

        assert!(!billing.is_configured());
        assert!(!billing.enforce);
    }

    #[test]
    fn hero_weights_default_to_the_spec_and_reject_a_negative_override() {
        let config = HeroConfig::from_env().unwrap();
        assert_eq!(config.fit_weights.player_performance, 0.30);

        env::set_var("FIT_WEIGHT_META", "-1");
        let rejected = HeroConfig::from_env();
        env::remove_var("FIT_WEIGHT_META");

        assert!(matches!(
            rejected,
            Err(ConfigError::Invalid("FIT_WEIGHT_META", _))
        ));
    }

    #[test]
    fn parsed_falls_back_when_unset_and_rejects_garbage() {
        // A name no test sets, so this exercises the unset path deterministically.
        assert_eq!(parsed::<u32>("DOTA_COACH_TEST_ABSENT_VAR", 20).unwrap(), 20);

        env::set_var("DOTA_COACH_TEST_BAD_VAR", "twenty");
        assert!(parsed::<u32>("DOTA_COACH_TEST_BAD_VAR", 20).is_err());
        env::remove_var("DOTA_COACH_TEST_BAD_VAR");
    }

    #[test]
    fn base_urls_never_keep_a_trailing_slash() {
        assert_eq!(trim_trailing_slash("http://x:3000/"), "http://x:3000");
        assert_eq!(trim_trailing_slash("http://x:3000"), "http://x:3000");
    }
}
