-- Reverse the case-insensitive identifier normalization.

ALTER TABLE community_membership_schedule DROP COLUMN email_normalized;
ALTER TABLE community_invites DROP COLUMN email_normalized;

DROP INDEX users_email_normalized_key;
DROP INDEX users_username_normalized_key;

ALTER TABLE users
DROP COLUMN email_normalized,
DROP COLUMN username_normalized;

-- Restore the original case-sensitive UNIQUE constraints.
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);
ALTER TABLE users ADD CONSTRAINT users_username_key UNIQUE (username);
