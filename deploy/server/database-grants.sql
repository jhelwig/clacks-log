-- Run as the schema owner after release migrations.
GRANT USAGE ON SCHEMA clacks TO clacks_runtime, clacks_maintenance;
-- UPDATE on accounts is required by PostgreSQL's account-row locking.
GRANT SELECT, INSERT, UPDATE ON clacks.accounts TO clacks_runtime;
GRANT SELECT, INSERT, DELETE ON clacks.login_identities TO clacks_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON clacks.login_sessions TO clacks_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON clacks.browser_sessions TO clacks_runtime;
GRANT SELECT ON clacks.server_administrators TO clacks_runtime;
GRANT SELECT, INSERT, UPDATE ON clacks.account_encryption_keys,
    clacks.sync_accounts, clacks.sync_baselines, clacks.sync_baseline_coverage,
    clacks.desktop_clients TO clacks_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON clacks.desktop_oauth_clients,
    clacks.desktop_client_grants, clacks.desktop_client_scopes,
    clacks.desktop_client_redirects, clacks.desktop_code_approvals,
    clacks.desktop_authorization_codes, clacks.desktop_code_scopes,
    clacks.desktop_code_resources, clacks.desktop_grants,
    clacks.desktop_access_tokens, clacks.desktop_access_scopes,
    clacks.desktop_access_resources, clacks.desktop_refresh_tokens,
    clacks.desktop_refresh_scopes, clacks.desktop_refresh_resources,
    clacks.oauth_revocations, clacks.oauth_replay_claims,
    clacks.resource_replay_claims TO clacks_runtime;
GRANT SELECT, UPDATE ON clacks.accounts TO clacks_maintenance;
GRANT SELECT, INSERT ON clacks.login_identities TO clacks_maintenance;
GRANT SELECT, INSERT, DELETE ON clacks.server_administrators TO clacks_maintenance;
