use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct LoginRateLimiter {
    inner: std::sync::Arc<Mutex<HashMap<String, Entry>>>,
}

#[derive(Clone)]
struct Entry {
    attempts: u32,
    window_started: Instant,
    blocked_until: Option<Instant>,
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self {
            inner: std::sync::Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn allow(&self, key: &str) -> bool {
        let mut map = self.inner.lock().expect("rate limiter");
        let now = Instant::now();
        let entry = map.entry(key.to_string()).or_insert(Entry {
            attempts: 0,
            window_started: now,
            blocked_until: None,
        });

        if let Some(blocked_until) = entry.blocked_until {
            if now < blocked_until {
                return false;
            }
            entry.blocked_until = None;
            entry.attempts = 0;
            entry.window_started = now;
        }

        if now.duration_since(entry.window_started) > Duration::from_secs(60) {
            entry.attempts = 0;
            entry.window_started = now;
        }

        true
    }

    pub fn record_failure(&self, key: &str) {
        let mut map = self.inner.lock().expect("rate limiter");
        let now = Instant::now();
        let entry = map.entry(key.to_string()).or_insert(Entry {
            attempts: 0,
            window_started: now,
            blocked_until: None,
        });
        if now.duration_since(entry.window_started) > Duration::from_secs(60) {
            entry.attempts = 0;
            entry.window_started = now;
        }
        entry.attempts += 1;
        if entry.attempts >= 8 {
            entry.blocked_until = Some(now + Duration::from_secs(300));
        }
    }

    pub fn record_success(&self, key: &str) {
        let mut map = self.inner.lock().expect("rate limiter");
        map.remove(key);
    }
}
