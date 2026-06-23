-- Case-insensitive uniqueness and lookups for user identifiers.
--
-- The username and email values are preserved exactly as the user entered
-- them. For emails this matters because RFC 5321 leaves the local part
-- (before the @) case-sensitive and owned by the receiving mail server, so
-- preserving the original casing ensures mail is delivered to the address the
-- user actually registered. For usernames it preserves the display casing the
-- user chose.
--
-- Uniqueness and lookups operate on a generated, lowercased "normalized" form
-- rather than the raw value. Generated columns are derived by the database, so
-- the normalized form can never drift from its source value regardless of
-- which write path inserts or updates the row.
--
-- The unique indexes are partial on `deleted_at IS NULL`, matching the existing
-- `idx_users_deleted_at` index and the `deleted_at IS NULL` filter that every
-- user lookup already applies. Soft-deleted rows are anonymized to unique
-- per-id values (`deleted-<id>@deleted.local`, `deleted-<id>`), so excluding
-- them keeps the index covering only the live identifier namespace rather than
-- indexing rows that can never be matched against.

-- Drop the existing case-sensitive UNIQUE constraints. They enforce uniqueness
-- on the raw value (so `Bob@x.com` and `bob@x.com` could coexist) and cannot be
-- partial, so they would also cover anonymized soft-deleted rows. Uniqueness is
-- re-established below on the normalized columns via partial indexes.
ALTER TABLE users DROP CONSTRAINT users_username_key;
ALTER TABLE users DROP CONSTRAINT users_email_key;

-- Generated normalized columns for users.
ALTER TABLE users
ADD COLUMN username_normalized VARCHAR(50)
    GENERATED ALWAYS AS (lower(username)) STORED,
ADD COLUMN email_normalized VARCHAR(255)
    GENERATED ALWAYS AS (lower(email)) STORED;

CREATE UNIQUE INDEX users_username_normalized_key
ON users (username_normalized)
WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX users_email_normalized_key
ON users (email_normalized)
WHERE deleted_at IS NULL;

-- Generated normalized column for invite emails. Invites match against a user's
-- email, so the match must compare normalized forms on both sides. The column
-- is nullable, matching the nullable `email` (an absent email means an open
-- invite to anyone with the invite id).
ALTER TABLE community_invites
ADD COLUMN email_normalized VARCHAR(255)
    GENERATED ALWAYS AS (lower(email)) STORED;

-- The membership schedule joins its email against users.email. Adding the
-- normalized column lets that join (and the per-row activity update) compare
-- normalized forms, so a scheduled `Bob@x.com` matches a registered
-- `bob@x.com`. The schema's documented hash-or-raw design for this column is
-- unaffected; only the raw-email matching path is made case-insensitive.
ALTER TABLE community_membership_schedule
ADD COLUMN email_normalized VARCHAR(255)
    GENERATED ALWAYS AS (lower(email)) STORED;
