//! レート制限
//!
//! IPアドレスごとの入室試行回数を一定時間窓で制限し、
//! ルームコードの総当たりや乱立を抑える。

use std::collections::HashMap;
use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 1つのIPが時間窓内に許される入室試行回数
const MAX_ATTEMPTS: usize = 10;

/// 時間窓の長さ
const WINDOW: Duration = Duration::from_secs(60);

/// 全体の掃除を始めるIP数のしきい値
///
/// 記録済みのIPがこの数を超えたら、時間窓を過ぎた古いIPの記録を
/// まとめて捨てる（IPごとの記録が無限に増えるのを防ぐ）。
const SWEEP_THRESHOLD: usize = 64;

/// IPごとの入室試行レート制限
#[derive(Clone, Default)]
pub struct RateLimiter {
    attempts: Arc<Mutex<HashMap<IpAddr, VecDeque<Instant>>>>,
}

impl RateLimiter {
    /// 新しいレート制限を作成する
    pub fn new() -> Self {
        RateLimiter::default()
    }

    /// 入室試行を1回記録し、許可されるかを返す
    ///
    /// 直近の時間窓内の試行が上限未満なら記録して `true` を返す。
    /// 上限に達していれば記録せず `false` を返す。
    pub fn check(&self, ip: IpAddr) -> bool {
        self.check_at(ip, Instant::now())
    }

    /// 時刻を指定して試行を判定する（テスト用）
    fn check_at(&self, ip: IpAddr, now: Instant) -> bool {
        let mut map = self.attempts.lock().unwrap();
        let cutoff = now.checked_sub(WINDOW);

        // IPが増えすぎたら、時間窓内の記録が無いIPをまとめて捨てる
        if map.len() > SWEEP_THRESHOLD
            && let Some(cutoff) = cutoff
        {
            map.retain(|_, entry| entry.back().is_some_and(|&last| last >= cutoff));
        }

        let entry = map.entry(ip).or_default();

        // 時間窓より古い記録を捨てる
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

    /// 記録されているIP数（テスト用）
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
        // 上限超過は拒否
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

        // 時間窓を超えて経過すると再び許可される
        let later = start + WINDOW + Duration::from_secs(1);
        assert!(limiter.check_at(ip(), later));
    }

    #[test]
    fn test_stale_ips_are_swept() {
        let limiter = RateLimiter::new();
        let start = Instant::now();

        // しきい値を超える数のIPを記録する
        for i in 0..=SWEEP_THRESHOLD {
            let ip: IpAddr = format!("10.0.{}.{}", i / 256, i % 256).parse().unwrap();
            assert!(limiter.check_at(ip, start));
        }
        assert!(limiter.tracked_ips() > SWEEP_THRESHOLD);

        // 時間窓を過ぎた後の試行で古いIPの記録が掃除される
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
        // 別IPは影響を受けない
        assert!(limiter.check_at(b, now));
    }
}
