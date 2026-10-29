CREATE SCHEMA clacks;
CREATE TABLE clacks.accounts (
    account_id TEXT PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE clacks.login_identities (
    identity_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES clacks.accounts(account_id),
    issuer TEXT COLLATE "C" NOT NULL,
    subject TEXT COLLATE "C" NOT NULL,
    display_name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT identity_issuer_subject UNIQUE (issuer, subject),
    UNIQUE (account_id, identity_id)
);
CREATE INDEX identities_account ON clacks.login_identities(account_id);
CREATE TABLE clacks.login_sessions (
    login_reference TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    identity_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    FOREIGN KEY (account_id, identity_id)
        REFERENCES clacks.login_identities(account_id, identity_id) ON DELETE CASCADE
);
CREATE INDEX logins_identity ON clacks.login_sessions(account_id, identity_id);
CREATE INDEX logins_expiration ON clacks.login_sessions(expires_at);
CREATE TABLE clacks.server_administrators (
    account_id TEXT PRIMARY KEY REFERENCES clacks.accounts(account_id),
    granted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE clacks.browser_sessions (
    id VARCHAR(128) PRIMARY KEY,
    expires BIGINT,
    session TEXT NOT NULL
);
