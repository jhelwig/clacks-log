SELECT c.account_id, c.client_id, c.redirect_uri, c.redirect_explicit,
c.code_challenge, c.key_thumbprint, c.issued_at AS "issued_at: jiff_sqlx::Timestamp", c.expires_at AS "expires_at: jiff_sqlx::Timestamp", c.state,
c.receipt,
COALESCE((SELECT string_agg(scope, ' ' ORDER BY scope) FROM clacks.desktop_code_scopes WHERE code_digest = c.code_digest), '') AS "scope!",
ARRAY(SELECT resource FROM clacks.desktop_code_resources WHERE code_digest = c.code_digest ORDER BY ordinal) AS "resources!"
FROM clacks.desktop_authorization_codes c WHERE code_digest = $1 FOR UPDATE OF c
