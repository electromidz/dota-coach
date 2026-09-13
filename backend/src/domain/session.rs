use rand::RngCore;
use sha2::{Digest, Sha256};

/// Session token entropy. 256 bits, hex-encoded into the cookie.
const TOKEN_BYTES: usize = 32;

/// Name of the session cookie.
pub const SESSION_COOKIE: &str = "dota_coach_session";

/// Name of the short-lived cookie holding the OpenID login nonce.
pub const LOGIN_STATE_COOKIE: &str = "dota_coach_login_state";

/// A freshly minted token: the plaintext goes to the browser exactly once, the
/// hash goes to the database.
pub struct NewToken {
    pub plaintext: String,
    pub hash: String,
}

impl NewToken {
    pub fn generate() -> Self {
        let mut bytes = [0u8; TOKEN_BYTES];
        rand::rng().fill_bytes(&mut bytes);

        let plaintext = hex::encode(bytes);
        let hash = hash_token(&plaintext);

        Self { plaintext, hash }
    }
}

/// Sessions are looked up by hash, so a database leak yields no usable cookies.
///
/// SHA-256 without a salt is deliberate: the input is 256 bits of CSPRNG
/// output, so there is no low-entropy space to brute force, and lookups must
/// stay a single indexed query.
pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unpredictable_and_hex_encoded() {
        let a = NewToken::generate();
        let b = NewToken::generate();

        assert_ne!(a.plaintext, b.plaintext);
        assert_eq!(a.plaintext.len(), TOKEN_BYTES * 2);
        assert!(a.plaintext.bytes().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn the_plaintext_token_is_never_the_stored_value() {
        let token = NewToken::generate();

        assert_ne!(token.plaintext, token.hash);
        assert_eq!(token.hash, hash_token(&token.plaintext));
        assert_eq!(token.hash.len(), 64);
    }

    #[test]
    fn hashing_is_stable_across_calls() {
        assert_eq!(hash_token("abc"), hash_token("abc"));
        assert_ne!(hash_token("abc"), hash_token("abd"));
    }
}
