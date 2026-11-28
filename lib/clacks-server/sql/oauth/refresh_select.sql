SELECT t.account_id,t.family_id,g.client_id,g.key_thumbprint,g.grant_established_at AS "grant_established_at: jiff_sqlx::Timestamp",
t.expires_at AS "expires_at: jiff_sqlx::Timestamp",
t.state AS token_state,
g.state AS family_state,g.provisional_until AS "provisional_until: jiff_sqlx::Timestamp",g.actor_id,c.closed_at AS "closed_at: jiff_sqlx::Timestamp",
COALESCE((SELECT string_agg(scope,' ' ORDER BY scope) FROM clacks.desktop_refresh_scopes WHERE refresh_digest=t.refresh_digest),'') AS "scope!",
ARRAY(SELECT resource FROM clacks.desktop_refresh_resources WHERE refresh_digest=t.refresh_digest ORDER BY ordinal) AS "resources!"
FROM clacks.desktop_refresh_tokens t JOIN clacks.desktop_grants g USING (account_id,family_id)
LEFT JOIN clacks.desktop_clients c ON c.account_id=g.account_id AND c.actor_id=g.actor_id
WHERE t.refresh_digest=$1 FOR UPDATE OF t,g
