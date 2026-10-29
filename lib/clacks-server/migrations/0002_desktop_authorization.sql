CREATE TABLE clacks.desktop_oauth_clients (
    client_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL
);
INSERT INTO clacks.desktop_oauth_clients VALUES ('clacks-desktop', 'Clacks Log desktop');
CREATE TABLE clacks.desktop_client_grants (
    client_id TEXT NOT NULL REFERENCES clacks.desktop_oauth_clients,
    grant_type TEXT NOT NULL,
    PRIMARY KEY (client_id, grant_type)
);
INSERT INTO clacks.desktop_client_grants VALUES
    ('clacks-desktop', 'authorization_code'), ('clacks-desktop', 'refresh_token');
CREATE TABLE clacks.desktop_client_scopes (
    client_id TEXT NOT NULL REFERENCES clacks.desktop_oauth_clients,
    scope TEXT NOT NULL,
    PRIMARY KEY (client_id, scope)
);
INSERT INTO clacks.desktop_client_scopes VALUES ('clacks-desktop', 'sync');
CREATE TABLE clacks.desktop_client_redirects (
    client_id TEXT NOT NULL REFERENCES clacks.desktop_oauth_clients,
    redirect_uri TEXT NOT NULL,
    PRIMARY KEY (client_id, redirect_uri)
);
INSERT INTO clacks.desktop_client_redirects VALUES
    ('clacks-desktop', 'http://127.0.0.1/oauth/callback'),
    ('clacks-desktop', 'http://[::1]/oauth/callback');

CREATE TABLE clacks.account_encryption_keys (
    account_id TEXT NOT NULL REFERENCES clacks.accounts,
    key_version BIGINT NOT NULL,
    wrapping_key_id TEXT NOT NULL,
    wrapping_nonce BYTEA NOT NULL,
    wrapped_key BYTEA NOT NULL,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (account_id, key_version)
);
CREATE UNIQUE INDEX account_active_key ON clacks.account_encryption_keys(account_id) WHERE active;

CREATE TABLE clacks.sync_baselines (
    account_id TEXT NOT NULL REFERENCES clacks.accounts,
    baseline_id TEXT NOT NULL,
    root_timeline TEXT NOT NULL,
    membership_generation BIGINT NOT NULL,
    PRIMARY KEY (account_id, baseline_id)
);
CREATE TABLE clacks.sync_baseline_coverage (
    account_id TEXT NOT NULL,
    baseline_id TEXT NOT NULL,
    timeline_id TEXT NOT NULL,
    revision BIGINT NOT NULL,
    PRIMARY KEY (account_id, baseline_id, timeline_id),
    FOREIGN KEY (account_id, baseline_id) REFERENCES clacks.sync_baselines ON DELETE CASCADE
);
CREATE TABLE clacks.sync_accounts (
    account_id TEXT PRIMARY KEY REFERENCES clacks.accounts,
    timeline_id TEXT NOT NULL,
    revision BIGINT NOT NULL,
    baseline_id TEXT NOT NULL,
    membership_generation BIGINT NOT NULL,
    FOREIGN KEY (account_id, baseline_id) REFERENCES clacks.sync_baselines
);
CREATE TABLE clacks.desktop_clients (
    account_id TEXT NOT NULL REFERENCES clacks.accounts,
    actor_id TEXT NOT NULL,
    label TEXT NOT NULL,
    authorized_at TIMESTAMPTZ NOT NULL,
    closed_at TIMESTAMPTZ,
    created_timeline TEXT NOT NULL,
    created_revision BIGINT NOT NULL,
    closed_timeline TEXT,
    closed_revision BIGINT,
    PRIMARY KEY (account_id, actor_id)
);
CREATE TABLE clacks.desktop_grants (
    account_id TEXT NOT NULL REFERENCES clacks.accounts,
    family_id TEXT NOT NULL UNIQUE,
    client_id TEXT NOT NULL REFERENCES clacks.desktop_oauth_clients,
    actor_id TEXT,
    grant_established_at TIMESTAMPTZ NOT NULL,
    key_thumbprint TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('provisional', 'active', 'revoked')),
    provisional_until TIMESTAMPTZ,
    PRIMARY KEY (account_id, family_id),
    FOREIGN KEY (account_id, actor_id) REFERENCES clacks.desktop_clients
);
CREATE INDEX desktop_grants_actor ON clacks.desktop_grants(account_id, actor_id);
CREATE INDEX desktop_grants_provisional ON clacks.desktop_grants(provisional_until) WHERE state = 'provisional';

CREATE TABLE clacks.desktop_code_approvals (
    code_digest BYTEA PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES clacks.accounts,
    purpose TEXT NOT NULL CHECK (purpose IN ('new', 'recover_active', 'replace_closed')),
    target_actor TEXT,
    label TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE TABLE clacks.desktop_authorization_codes (
    code_digest BYTEA PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES clacks.accounts,
    client_id TEXT NOT NULL REFERENCES clacks.desktop_oauth_clients,
    redirect_uri TEXT NOT NULL,
    redirect_explicit BOOLEAN NOT NULL,
    code_challenge TEXT NOT NULL,
    key_thumbprint TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('issued', 'consumed', 'replayed')),
    receipt_key_version BIGINT,
    receipt_nonce BYTEA,
    receipt_ciphertext BYTEA,
    FOREIGN KEY (account_id, receipt_key_version) REFERENCES clacks.account_encryption_keys
);
CREATE INDEX desktop_codes_expiration ON clacks.desktop_authorization_codes(expires_at);
CREATE TABLE clacks.desktop_code_scopes (
    code_digest BYTEA NOT NULL REFERENCES clacks.desktop_authorization_codes ON DELETE CASCADE,
    scope TEXT NOT NULL,
    PRIMARY KEY (code_digest, scope)
);
CREATE TABLE clacks.desktop_code_resources (
    code_digest BYTEA NOT NULL REFERENCES clacks.desktop_authorization_codes ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    resource TEXT NOT NULL,
    PRIMARY KEY (code_digest, ordinal)
);

CREATE TABLE clacks.desktop_access_tokens (
    access_digest BYTEA PRIMARY KEY,
    account_id TEXT NOT NULL,
    family_id TEXT NOT NULL,
    client_id TEXT NOT NULL REFERENCES clacks.desktop_oauth_clients,
    key_thumbprint TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL,
    grant_established_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    FOREIGN KEY (account_id, family_id) REFERENCES clacks.desktop_grants ON DELETE CASCADE
);
CREATE INDEX desktop_access_expiration ON clacks.desktop_access_tokens(expires_at);
CREATE TABLE clacks.desktop_access_scopes (
    access_digest BYTEA NOT NULL REFERENCES clacks.desktop_access_tokens ON DELETE CASCADE,
    scope TEXT NOT NULL,
    PRIMARY KEY (access_digest, scope)
);
CREATE TABLE clacks.desktop_access_resources (
    access_digest BYTEA NOT NULL REFERENCES clacks.desktop_access_tokens ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    resource TEXT NOT NULL,
    PRIMARY KEY (access_digest, ordinal)
);
CREATE TABLE clacks.desktop_refresh_tokens (
    refresh_digest BYTEA PRIMARY KEY,
    account_id TEXT NOT NULL,
    family_id TEXT NOT NULL,
    client_id TEXT NOT NULL REFERENCES clacks.desktop_oauth_clients,
    key_thumbprint TEXT NOT NULL,
    grant_established_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ,
    state TEXT NOT NULL CHECK (state IN ('active', 'spent')),
    FOREIGN KEY (account_id, family_id) REFERENCES clacks.desktop_grants ON DELETE CASCADE
);
CREATE TABLE clacks.desktop_refresh_scopes (
    refresh_digest BYTEA NOT NULL REFERENCES clacks.desktop_refresh_tokens ON DELETE CASCADE,
    scope TEXT NOT NULL,
    PRIMARY KEY (refresh_digest, scope)
);
CREATE TABLE clacks.desktop_refresh_resources (
    refresh_digest BYTEA NOT NULL REFERENCES clacks.desktop_refresh_tokens ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    resource TEXT NOT NULL,
    PRIMARY KEY (refresh_digest, ordinal)
);
CREATE TABLE clacks.oauth_revocations (
    scope TEXT NOT NULL,
    target TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    until_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (scope, target)
);
CREATE TABLE clacks.oauth_replay_claims (
    replay_id TEXT PRIMARY KEY,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE TABLE clacks.resource_replay_claims (
    key_thumbprint TEXT NOT NULL,
    proof_id TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (key_thumbprint, proof_id)
);

-- Family revocation and bounded expiry sweeps use these ordinary access paths.
CREATE INDEX desktop_approvals_expiration ON clacks.desktop_code_approvals(expires_at);
CREATE INDEX desktop_access_family ON clacks.desktop_access_tokens(family_id);
CREATE INDEX desktop_refresh_family ON clacks.desktop_refresh_tokens(family_id);
CREATE INDEX desktop_refresh_expiration ON clacks.desktop_refresh_tokens(expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX oauth_revocations_expiration ON clacks.oauth_revocations(until_at);
CREATE INDEX oauth_replay_expiration ON clacks.oauth_replay_claims(expires_at);
CREATE INDEX resource_replay_expiration ON clacks.resource_replay_claims(expires_at);
