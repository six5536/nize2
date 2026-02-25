// @awa-component: DB-UUIDv7
// Helper for generating UUIDv7 (timestamp-sortable UUIDs)
//
// PostgreSQL does not natively support UUIDv7 for auto-generation.
// For tables where time-ordering matters (conversations, messages,
// MCP servers, audit logs, etc.), we generate UUIDv7 app-side.
// Tables where time-ordering is irrelevant (users, config_values)
// continue to use PG's gen_random_uuid() (v4).

use uuid::Uuid;

/// Generate a new UUIDv7 (timestamp-sortable).
pub fn uuidv7() -> Uuid {
    Uuid::now_v7()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuidv7_is_valid() {
        let id = uuidv7();
        assert_eq!(id.get_version(), Some(uuid::Version::SortRand));
    }

    #[test]
    fn uuidv7_is_monotonic() {
        let a = uuidv7();
        let b = uuidv7();
        // UUIDv7 embeds timestamp — later IDs sort after earlier ones
        assert!(b >= a);
    }
}
