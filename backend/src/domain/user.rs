use chrono::{DateTime, Utc};
use serde::{Serialize, Serializer};
use utoipa::ToSchema;
use uuid::Uuid;

/// An application account.
///
/// `steam_id` is written only from a verified OpenID assertion. Nothing in the
/// request body can reach it.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct User {
    pub id: Uuid,
    /// Serialized as a string: SteamID64 exceeds JavaScript's safe integer range.
    #[serde(serialize_with = "as_string")]
    pub steam_id: i64,
    pub persona_name: Option<String>,
    pub avatar_url: Option<String>,
    pub profile_url: Option<String>,
    pub last_login_at: Option<DateTime<Utc>>,
    /// Gates the admin panel. Set only by hand in the database — there is no
    /// self-serve promotion path.
    pub is_admin: bool,
    /// `active` or `disabled`, enforced by the `CHECK` constraint in
    /// `migrations/0014_admin.sql`. Kept as the raw stored value rather than a
    /// parsed enum: `User` is decoded directly by every query that reads one,
    /// and two known values don't earn a third representation.
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// A disabled account is refused everywhere `CurrentUser` is resolved —
    /// see `api::extract::CurrentUser`. Checked here rather than inline at
    /// every call site so "what does disabled mean" has one answer.
    pub fn is_disabled(&self) -> bool {
        self.status == "disabled"
    }
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
