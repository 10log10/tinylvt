-- Add up migration script here

CREATE TABLE communities (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE
);

CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    display_name VARCHAR(255),
    -- if email ownership has been verified
    email_verified BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TYPE community_user_role AS ENUM (
    'Leader',  -- only one leader
    'CoLeader',  -- same privileges as leader, but can have multiple
    'Moderator',  -- lower-level privileges, but above member
    'Member'  -- default membership level
);

CREATE TABLE community_members (
    -- cascade: if a community is deleted, memberships are deleted too
    community_id INTEGER REFERENCES communities (id) ON DELETE CASCADE,
    user_id INTEGER REFERENCES users (id) ON DELETE CASCADE,
    role COMMUNITY_USER_ROLE NOT NULL,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- time of last activity in this community
    active_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (community_id, user_id)
);

CREATE UNIQUE INDEX one_leader_per_community
ON community_members (community_id)
WHERE role = 'Leader';
