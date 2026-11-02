-- The only public application registration is release-owned server policy.
-- Individual grants and codes retain their actual application identifiers.
DROP TABLE clacks.desktop_client_redirects;
DROP TABLE clacks.desktop_client_grants;
DROP TABLE clacks.desktop_client_scopes;
ALTER TABLE clacks.desktop_grants
    DROP CONSTRAINT desktop_grants_client_id_fkey;
ALTER TABLE clacks.desktop_authorization_codes
    DROP CONSTRAINT desktop_authorization_codes_client_id_fkey;
DROP TABLE clacks.desktop_oauth_clients;
