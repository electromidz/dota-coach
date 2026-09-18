//! A per-user sliding-window throttle for voucher redemption attempts.
//!
//! In-memory rather than a stored counter, on purpose: a rejected guess is
//! not a product fact worth an `events` row — nobody needs "how many times
//! did this account mistype a code" in the analytics timeline — so this
//! keeps abuse-prevention noise entirely out of the database. The trade-off
//! is that the limit resets on a restart and is not shared across multiple
//! backend instances; acceptable for a single-instance deployment guarding
//! against a human fat-fingering codes, not a distributed attack.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use uuid::Uuid;

const MAX_ATTEMPTS: usize = 5;
const WINDOW: Duration = Duration::from_secs(60);

pub struct RateLimiter {
    attempts: Mutex<HashMap<Uuid, VecDeque<Instant>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
        }
    }

    /// Records this attempt and reports whether it may proceed. Recording
    /// happens whether or not it is allowed — an attempt that doesn't count
    /// against the window could otherwise be repeated indefinitely right at
    /// the boundary.
    pub fn check(&self, user_id: Uuid) -> bool {
        let now = Instant::now();
        let mut attempts = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        let window = attempts.entry(user_id).or_default();

        while window.front().is_some_and(|&t| now.duration_since(t) > WINDOW) {
            window.pop_front();
        }

        if window.len() >= MAX_ATTEMPTS {
            return false;
        }

        window.push_back(now);
        true
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sixth_attempt_within_a_minute_is_refused() {
        let limiter = RateLimiter::new();
        let user_id = Uuid::new_v4();

        for _ in 0..5 {
            assert!(limiter.check(user_id));
        }
        assert!(!limiter.check(user_id), "the sixth attempt should be refused");
    }

    #[test]
    fn different_users_have_independent_windows() {
        let limiter = RateLimiter::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        for _ in 0..5 {
            assert!(limiter.check(a));
        }
        assert!(!limiter.check(a));
        // B has made no attempts yet, so A's exhausted window must not
        // borrow against B's.
        assert!(limiter.check(b));
    }
}
