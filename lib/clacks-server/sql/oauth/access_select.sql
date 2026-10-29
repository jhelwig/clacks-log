SELECT t.account_id,t.family_id,t.client_id,t.key_thumbprint,t.grant_established_at,
t.issued_at AS issued_at, t.expires_at,
NULL::text AS token_state,
g.state AS family_state,g.actor_id,c.closed_at,
COALESCE((SELECT string_agg(scope,' ' ORDER BY scope) FROM clacks.desktop_access_scopes WHERE access_digest=t.access_digest),'') AS "scope!",
ARRAY(SELECT resource FROM clacks.desktop_access_resources WHERE access_digest=t.access_digest ORDER BY ordinal) AS "resources!"
FROM clacks.desktop_access_tokens t JOIN clacks.desktop_grants g USING (account_id,family_id)
LEFT JOIN clacks.desktop_clients c ON c.account_id=g.account_id AND c.actor_id=g.actor_id
WHERE t.access_digest=$1 FOR UPDATE OF t,g
