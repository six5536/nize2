// @zen-component: CFG-ConfigCache
//
//! In-memory config cache with TTL-based expiration.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

/// Default TTL for system config: 5 minutes.
pub const DEFAULT_SYSTEM_TTL_MS: i64 = 300_000;

/// Default TTL for user-override config: 30 seconds.
pub const DEFAULT_USER_OVERRIDE_TTL_MS: i64 = 30_000;

/// A cached entry with expiry.
#[derive(Debug, Clone)]
struct CacheEntry {
    value: String,
    expires_at: DateTime<Utc>,
}

/// In-memory config cache keyed by `(config_key, scope, user_id)`.
#[derive(Debug)]
pub struct ConfigCache {
    entries: HashMap<String, CacheEntry>,
    /// TTL for system scope entries (milliseconds).
    pub system_ttl_ms: i64,
    /// TTL for user-override scope entries (milliseconds).
    pub user_override_ttl_ms: i64,
}

impl ConfigCache {
    /// Create a new cache with default TTLs.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            system_ttl_ms: DEFAULT_SYSTEM_TTL_MS,
            user_override_ttl_ms: DEFAULT_USER_OVERRIDE_TTL_MS,
        }
    }

    /// Build a composite cache key.
    fn cache_key(key: &str, scope: &str, user_id: Option<&str>) -> String {
        match user_id {
            Some(uid) => format!("{key}:{scope}:{uid}"),
            None => format!("{key}:{scope}:_"),
        }
    }

    /// Get a cached value if it exists and has not expired.
    pub fn get(&self, key: &str, scope: &str, user_id: Option<&str>) -> Option<String> {
        let ck = Self::cache_key(key, scope, user_id);
        self.entries.get(&ck).and_then(|entry| {
            if Utc::now() < entry.expires_at {
                Some(entry.value.clone())
            } else {
                None
            }
        })
    }

    /// Insert or update a cached value.
    pub fn set(&mut self, key: &str, scope: &str, user_id: Option<&str>, value: String) {
        let ck = Self::cache_key(key, scope, user_id);
        let ttl_ms = match scope {
            "system" => self.system_ttl_ms,
            _ => self.user_override_ttl_ms,
        };
        let expires_at = Utc::now() + chrono::Duration::milliseconds(ttl_ms);
        self.entries.insert(ck, CacheEntry { value, expires_at });
    }

    /// Remove a specific entry from the cache.
    pub fn invalidate(&mut self, key: &str, scope: &str, user_id: Option<&str>) {
        let ck = Self::cache_key(key, scope, user_id);
        self.entries.remove(&ck);
    }

    /// Remove all cache entries for a given config key (all scopes, all users).
    pub fn invalidate_all_for_key(&mut self, key: &str) {
        self.entries
            .retain(|ck, _| !ck.starts_with(&format!("{key}:")));
    }

    /// Remove all entries from the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for ConfigCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_none_for_missing_key() {
        let cache = ConfigCache::new();
        assert!(cache.get("unknown", "system", None).is_none());
    }

    #[test]
    fn set_and_get_roundtrip() {
        let mut cache = ConfigCache::new();
        cache.set("k1", "system", None, "val1".to_string());
        assert_eq!(cache.get("k1", "system", None), Some("val1".to_string()));
    }

    #[test]
    fn set_with_user_id() {
        let mut cache = ConfigCache::new();
        cache.set("k1", "user-override", Some("u1"), "val_u1".to_string());
        cache.set("k1", "user-override", Some("u2"), "val_u2".to_string());
        assert_eq!(
            cache.get("k1", "user-override", Some("u1")),
            Some("val_u1".to_string())
        );
        assert_eq!(
            cache.get("k1", "user-override", Some("u2")),
            Some("val_u2".to_string())
        );
    }

    #[test]
    fn invalidate_removes_specific_entry() {
        let mut cache = ConfigCache::new();
        cache.set("k1", "system", None, "val1".to_string());
        cache.set("k2", "system", None, "val2".to_string());
        cache.invalidate("k1", "system", None);
        assert!(cache.get("k1", "system", None).is_none());
        assert_eq!(cache.get("k2", "system", None), Some("val2".to_string()));
    }

    #[test]
    fn invalidate_all_for_key_removes_all_scopes() {
        let mut cache = ConfigCache::new();
        cache.set("k1", "system", None, "sys".to_string());
        cache.set("k1", "user-override", Some("u1"), "usr".to_string());
        cache.set("k2", "system", None, "other".to_string());
        cache.invalidate_all_for_key("k1");
        assert!(cache.get("k1", "system", None).is_none());
        assert!(cache.get("k1", "user-override", Some("u1")).is_none());
        assert_eq!(cache.get("k2", "system", None), Some("other".to_string()));
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut cache = ConfigCache::new();
        cache.set("k1", "system", None, "v1".to_string());
        cache.set("k2", "system", None, "v2".to_string());
        cache.clear();
        assert!(cache.get("k1", "system", None).is_none());
        assert!(cache.get("k2", "system", None).is_none());
    }

    #[test]
    fn expired_entry_returns_none() {
        let mut cache = ConfigCache::new();
        // Set TTL to 0 so it expires immediately
        cache.system_ttl_ms = 0;
        cache.set("k1", "system", None, "val1".to_string());
        // Should be expired
        assert!(cache.get("k1", "system", None).is_none());
    }
}
