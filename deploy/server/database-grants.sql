-- Run as the schema owner after release migrations.
GRANT USAGE ON SCHEMA clacks TO clacks_runtime, clacks_maintenance;
-- UPDATE on accounts is required by PostgreSQL's account-row locking.
GRANT SELECT, INSERT, UPDATE ON clacks.accounts TO clacks_runtime;
GRANT SELECT, INSERT, DELETE ON clacks.login_identities TO clacks_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON clacks.login_sessions TO clacks_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON clacks.browser_sessions TO clacks_runtime;
GRANT SELECT ON clacks.server_administrators TO clacks_runtime;
GRANT SELECT, UPDATE ON clacks.accounts TO clacks_maintenance;
GRANT SELECT, INSERT ON clacks.login_identities TO clacks_maintenance;
GRANT SELECT, INSERT, DELETE ON clacks.server_administrators TO clacks_maintenance;
