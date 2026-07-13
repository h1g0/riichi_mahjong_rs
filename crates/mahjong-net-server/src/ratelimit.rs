//! Rate limiting: caps join attempts per IP within a time window to
//! deter room-code brute forcing and room spam.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Join attempts allowed per IP within the window.
const MAX_ATTEMPTS: usize = 10;

/// Window length.
const WINDOW: Duration = Duration::from_secs(60);

/// IP-count threshold that triggers a sweep.
///
/// Once more IPs than this are tracked, entries whose attempts all fall
/// outside the window are dropped in bulk, bounding memory.
const SWEEP_THRESHOLD: usize = 64;

/// Per-IP join-attempt rate limiter.
#[derive(Clone, Default)]
pub struct RateLimiter {
    attempts: Arc<Mutex<HashMap<IpAddr, VecDeque<Instant>>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        RateLimiter::default()
    }

    /// Records one join attempt and returns whether it is allowed:
    /// below the in-window cap it is recorded and `true` returns;
    /// at the cap nothing is recorded and `false` returns.
    pub fn check(&self, ip: IpAddr) -> bool {
        self.check_at(ip, Instant::now())
    }

    /// Attempt check with an explicit time (for tests).
    fn check_at(&self, ip: IpAddr, now: Instant) -> bool {
        let mut map = self.attempts.lock().unwrap();
        let cutoff = now.checked_sub(WINDOW);

        // Sweep idle IPs once too many are tracked.
        if map.len() > SWEEP_THRESHOLD
            && let Some(cutoff) = cutoff
        {
            map.retain(|_, entry| entry.back().is_some_and(|&last| last >= cutoff));
        }

        let entry = map.entry(ip).or_default();

        // Drop attempts older than the window.
        while let Some(&front) = entry.front() {
            match cutoff {
                Some(cutoff) if front < cutoff => {
                    entry.pop_front();
                }
                _ => break,
            }
        }

        if entry.len() >= MAX_ATTEMPTS {
            return false;
        }
        entry.push_back(now);
        true
    }

    /// Number of tracked IPs (for tests).
    #[cfg(test)]
    fn tracked_ips(&self) -> usize {
        self.attempts.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip() -> IpAddr {
        "127.0.0.1".parse().unwrap()
    }

    #[test]
    fn test_allows_up_to_limit_then_blocks() {
        let limiter = RateLimiter::new();
        let now = Instant::now();
        for i in 0..MAX_ATTEMPTS {
            assert!(limiter.check_at(ip(), now), "{i}回目は許可されるべき");
        }

        assert!(!limiter.check_at(ip(), now), "上限超過は拒否されるべき");
    }

    #[test]
    fn test_window_expiry_resets() {
        let limiter = RateLimiter::new();
        let start = Instant::now();
        for _ in 0..MAX_ATTEMPTS {
            assert!(limiter.check_at(ip(), start));
        }
        assert!(!limiter.check_at(ip(), start));

        // Past the window, attempts are allowed again.
        let later = start + WINDOW + Duration::from_secs(1);
        assert!(limiter.check_at(ip(), later));
    }

    #[test]
    fn test_stale_ips_are_swept() {
        let limiter = RateLimiter::new();
        let start = Instant::now();

        // Track more IPs than the sweep threshold.
        for i in 0..=SWEEP_THRESHOLD {
            let ip: IpAddr = format!("10.0.{}.{}", i / 256, i % 256).parse().unwrap();
            assert!(limiter.check_at(ip, start));
        }
        assert!(limiter.tracked_ips() > SWEEP_THRESHOLD);

        // An attempt after the window sweeps the stale IPs.
        let later = start + WINDOW + Duration::from_secs(1);
        assert!(limiter.check_at(ip(), later));
        assert_eq!(limiter.tracked_ips(), 1, "古いIPの記録が残っている");
    }

    #[test]
    fn test_separate_ips_are_independent() {
        let limiter = RateLimiter::new();
        let now = Instant::now();
        let a: IpAddr = "10.0.0.1".parse().unwrap();
        let b: IpAddr = "10.0.0.2".parse().unwrap();
        for _ in 0..MAX_ATTEMPTS {
            assert!(limiter.check_at(a, now));
        }
        assert!(!limiter.check_at(a, now));
        // Other IPs are unaffected.
        assert!(limiter.check_at(b, now));
    }
}
