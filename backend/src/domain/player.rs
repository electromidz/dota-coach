use chrono::{DateTime, Utc};
use serde::{Serialize, Serializer};
use uuid::Uuid;

/// SteamID64 = 32-bit Dota account id + this constant.
pub const STEAM_ID64_BASE: i64 = 76_561_197_960_265_728;

/// Account ids are 32-bit; anything larger is either a SteamID64 or nonsense.
const MAX_ACCOUNT_ID: i64 = u32::MAX as i64;

/// The Dota identity linked to an application account. One row per account.
///
/// Derived from the account's proven SteamID64, never from client input.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DotaPlayer {
    pub id: Uuid,
    pub user_id: Uuid,
    /// Serialized as a string: SteamID64 exceeds JavaScript's safe integer range.
    #[serde(serialize_with = "as_string")]
    pub steam_id: i64,
    pub dota_account_id: i64,
    pub rank_tier: Option<i32>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn as_string<S: Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

/// The two equivalent spellings of the same account, resolved once so nothing
/// downstream has to guess which form it was handed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerIdentity {
    pub steam_id: i64,
    pub account_id: i64,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum IdentityError {
    #[error("That Steam account has no Dota player id.")]
    OutOfRange,
}

impl PlayerIdentity {
    /// Derive the Dota identity from an authenticated SteamID64.
    ///
    /// This is the only way a `PlayerIdentity` is built. There is deliberately
    /// no constructor that parses a client-supplied string: the Steam id always
    /// comes from a verified OpenID assertion.
    pub fn from_steam_id(steam_id: i64) -> Result<Self, IdentityError> {
        let account_id = steam_id - STEAM_ID64_BASE;
        if account_id <= 0 || account_id > MAX_ACCOUNT_ID {
            return Err(IdentityError::OutOfRange);
        }

        Ok(Self {
            steam_id,
            account_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_authenticated_steam_id_maps_to_its_account_id() {
        let identity = PlayerIdentity::from_steam_id(76_561_198_047_011_640).unwrap();

        assert_eq!(identity.account_id, 86_745_912);
        assert_eq!(identity.steam_id, 76_561_198_047_011_640);
    }

    #[test]
    fn a_steam_id_at_or_below_the_base_has_no_dota_account() {
        assert_eq!(
            PlayerIdentity::from_steam_id(STEAM_ID64_BASE),
            Err(IdentityError::OutOfRange)
        );
        assert_eq!(
            PlayerIdentity::from_steam_id(123),
            Err(IdentityError::OutOfRange)
        );
        assert_eq!(
            PlayerIdentity::from_steam_id(0),
            Err(IdentityError::OutOfRange)
        );
    }

    #[test]
    fn a_steam_id_beyond_the_account_id_ceiling_is_rejected() {
        assert_eq!(
            PlayerIdentity::from_steam_id(STEAM_ID64_BASE + MAX_ACCOUNT_ID + 1),
            Err(IdentityError::OutOfRange)
        );
    }
}
