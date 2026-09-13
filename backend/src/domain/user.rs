use chrono::{DateTime, Utc};
use serde::{Serialize, Serializer};
use uuid::Uuid;

/// An application account.
///
/// `steam_id` is written only from a verified OpenID assertion. Nothing in the
/// request body can reach it.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    /// Serialized as a string: SteamID64 exceeds JavaScript's safe integer range.
    #[serde(serialize_with = "as_string")]
    pub steam_id: i64,
    pub persona_name: Option<String>,
    pub avatar_url: Option<String>,
    pub profile_url: Option<String>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn as_string<S: Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

/// Steam profile fields, refreshed from the Dota provider. `None` leaves the
/// stored value untouched so a temporarily private profile does not wipe data.
#[derive(Debug, Clone, Default)]
pub struct SteamProfileUpdate {
    pub persona_name: Option<String>,
    pub avatar_url: Option<String>,
    pub profile_url: Option<String>,
}
