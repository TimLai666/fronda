//! Stateful MCP session management (upstream #250).
//!
//! An MCP client establishes a session on `initialize`, receives an
//! `Mcp-Session-Id`, and routes later requests to it. This module owns the pure,
//! deterministic pieces of that model: an LRU + TTL [`SessionStore`] keyed by
//! session id, and header parsing. Time is passed in explicitly so eviction and
//! pruning are unit-testable. Transport, SSE streaming, and `tools/list_changed`
//! notifications are wired in the server layer.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Maximum live sessions before the least-recently-used is evicted (#250).
pub const DEFAULT_SESSION_CAPACITY: usize = 32;

/// Idle lifetime after which a session is pruned (#250: 1 hour).
pub const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(3600);

struct Entry<T> {
    value: T,
    last_access: Instant,
    /// Monotonic access counter: a stable secondary LRU key so two entries sharing
    /// one `last_access` Instant (coarse clock resolution) evict deterministically
    /// instead of by nondeterministic HashMap iteration order.
    seq: u64,
}

/// A bounded, idle-expiring registry of per-session state. Capacity is enforced
/// by LRU eviction on insert; the TTL is enforced by [`prune_expired`]. Access
/// times advance on `insert` and `get`.
pub struct SessionStore<T> {
    capacity: usize,
    ttl: Duration,
    entries: HashMap<String, Entry<T>>,
    next_seq: u64,
}

impl<T> SessionStore<T> {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            capacity: capacity.max(1),
            ttl,
            entries: HashMap::new(),
            next_seq: 0,
        }
    }

    /// Store with the #250 defaults: LRU(32), 1-hour TTL.
    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_SESSION_CAPACITY, DEFAULT_SESSION_TTL)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    /// Insert or replace a session, marking it most-recently-used. If this adds
    /// a new session beyond capacity, the least-recently-used session is evicted
    /// first; its id is returned. Replacing an existing id never evicts.
    pub fn insert(&mut self, id: String, value: T, now: Instant) -> Option<String> {
        let mut evicted = None;
        if !self.entries.contains_key(&id) && self.entries.len() >= self.capacity {
            if let Some(lru) = self.lru_id() {
                self.entries.remove(&lru);
                evicted = Some(lru);
            }
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.entries.insert(
            id,
            Entry {
                value,
                last_access: now,
                seq,
            },
        );
        evicted
    }

    /// Access a session, refreshing its recency. `None` if absent.
    pub fn get(&mut self, id: &str, now: Instant) -> Option<&mut T> {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        let entry = self.entries.get_mut(id)?;
        entry.last_access = now;
        entry.seq = seq;
        Some(&mut entry.value)
    }

    /// Remove a session explicitly (e.g. client `DELETE`).
    pub fn remove(&mut self, id: &str) -> Option<T> {
        self.entries.remove(id).map(|e| e.value)
    }

    /// Visit every session value mutably; entries for which the closure
    /// returns false are removed. Access times are NOT refreshed (a broadcast
    /// is not client activity).
    pub fn retain_values(&mut self, mut keep: impl FnMut(&mut T) -> bool) {
        self.entries.retain(|_, entry| keep(&mut entry.value));
    }

    /// Drop sessions whose idle time exceeds the TTL. Returns the removed ids.
    pub fn prune_expired(&mut self, now: Instant) -> Vec<String> {
        let ttl = self.ttl;
        let expired: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, e)| now.duration_since(e.last_access) > ttl)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &expired {
            self.entries.remove(id);
        }
        expired
    }

    fn lru_id(&self) -> Option<String> {
        self.entries
            .iter()
            .min_by_key(|(_, e)| (e.last_access, e.seq))
            .map(|(id, _)| id.clone())
    }
}

/// Extract the `Mcp-Session-Id` header value from a raw HTTP request, if present.
/// Header names are case-insensitive (RFC 7230); the value is trimmed.
pub fn parse_session_id(raw_http_request: &str) -> Option<String> {
    let headers = raw_http_request.split("\r\n\r\n").next()?;
    for line in headers.split("\r\n").skip(1) {
        // Skip a colon-less line rather than `?`-aborting the whole scan (which would
        // miss a valid Mcp-Session-Id header appearing after it).
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("mcp-session-id") {
            let v = value.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn insert_and_get_within_capacity() {
        let mut store: SessionStore<i32> = SessionStore::new(3, DEFAULT_SESSION_TTL);
        let now = t0();
        assert_eq!(store.insert("a".into(), 1, now), None);
        assert_eq!(store.insert("b".into(), 2, now), None);
        assert_eq!(store.len(), 2);
        assert_eq!(store.get("a", now), Some(&mut 1));
        assert!(store.get("missing", now).is_none());
    }

    #[test]
    fn insert_over_capacity_evicts_lru() {
        let mut store: SessionStore<i32> = SessionStore::new(2, DEFAULT_SESSION_TTL);
        let base = t0();
        store.insert("a".into(), 1, base);
        store.insert("b".into(), 2, base + Duration::from_secs(1));
        // Touch "a" so "b" becomes the least-recently-used.
        store.get("a", base + Duration::from_secs(2));
        let evicted = store.insert("c".into(), 3, base + Duration::from_secs(3));
        assert_eq!(evicted.as_deref(), Some("b"));
        assert!(store.contains("a"));
        assert!(store.contains("c"));
        assert!(!store.contains("b"));
    }

    #[test]
    fn lru_tie_break_is_deterministic_on_equal_timestamps() {
        // Two sessions inserted at the SAME Instant, then "a" is accessed at that same
        // Instant. On overflow the true LRU ("b") must evict — not a nondeterministic
        // HashMap pick that could drop the just-accessed "a".
        let mut store: SessionStore<i32> = SessionStore::new(2, DEFAULT_SESSION_TTL);
        let now = t0();
        store.insert("a".into(), 1, now);
        store.insert("b".into(), 2, now);
        store.get("a", now); // "a" becomes most-recent by seq; "b" is the LRU
        let evicted = store.insert("c".into(), 3, now);
        assert_eq!(
            evicted.as_deref(),
            Some("b"),
            "true LRU evicted on a timestamp tie"
        );
        assert!(store.contains("a"));
        assert!(store.contains("c"));
    }

    #[test]
    fn parse_session_id_skips_colonless_lines() {
        // A malformed colon-less header line before the real one must not abort the
        // scan (the old `?` returned None for the whole request).
        let raw = "POST /mcp HTTP/1.1\r\nHost: x\r\nMalformedNoColon\r\nMcp-Session-Id: abc123\r\n\r\nbody";
        assert_eq!(parse_session_id(raw), Some("abc123".to_string()));
    }

    #[test]
    fn replacing_existing_id_never_evicts() {
        let mut store: SessionStore<i32> = SessionStore::new(2, DEFAULT_SESSION_TTL);
        let now = t0();
        store.insert("a".into(), 1, now);
        store.insert("b".into(), 2, now);
        let evicted = store.insert("a".into(), 9, now);
        assert_eq!(evicted, None);
        assert_eq!(store.len(), 2);
        assert_eq!(store.get("a", now), Some(&mut 9));
    }

    #[test]
    fn prune_expired_drops_idle_sessions() {
        let mut store: SessionStore<i32> = SessionStore::new(8, Duration::from_secs(60));
        let base = t0();
        store.insert("old".into(), 1, base);
        store.insert("fresh".into(), 2, base + Duration::from_secs(120));
        let removed = store.prune_expired(base + Duration::from_secs(121));
        assert_eq!(removed, vec!["old".to_string()]);
        assert!(store.contains("fresh"));
        assert!(!store.contains("old"));
    }

    #[test]
    fn prune_keeps_recently_accessed() {
        let mut store: SessionStore<i32> = SessionStore::new(8, Duration::from_secs(60));
        let base = t0();
        store.insert("a".into(), 1, base);
        // Access refreshes recency, so it survives a later prune.
        store.get("a", base + Duration::from_secs(90));
        let removed = store.prune_expired(base + Duration::from_secs(100));
        assert!(removed.is_empty());
        assert!(store.contains("a"));
    }

    #[test]
    fn parses_session_id_case_insensitively() {
        let req = "POST /mcp HTTP/1.1\r\nHost: x\r\nMCP-SESSION-ID:  abc-123 \r\n\r\n{}";
        assert_eq!(parse_session_id(req).as_deref(), Some("abc-123"));
    }

    #[test]
    fn missing_session_id_is_none() {
        let req = "POST /mcp HTTP/1.1\r\nHost: x\r\n\r\n{}";
        assert_eq!(parse_session_id(req), None);
    }
}
