-- Extend transport_type enum with SSE and managed transport variants.
-- Note: ALTER TYPE ... ADD VALUE cannot run inside a transaction block in PostgreSQL.
-- SQLx runs each migration file as a single transaction, but ADD VALUE IF NOT EXISTS
-- is safe to use here because PGlite and modern PostgreSQL handle this gracefully.

ALTER TYPE transport_type ADD VALUE IF NOT EXISTS 'sse';
ALTER TYPE transport_type ADD VALUE IF NOT EXISTS 'managed-sse';
ALTER TYPE transport_type ADD VALUE IF NOT EXISTS 'managed-http';
