SELECT t.account_id,t.family_id,g.client_id,g.key_thumbprint,g.grant_established_at AS "grant_established_at: jiff_sqlx::Timestamp",
t.issued_at AS "issued_at: jiff_sqlx::Timestamp",t.expires_at AS "expires_at: jiff_sqlx::Timestamp",g.state AS family_state,g.provisional_until AS "provisional_until: jiff_sqlx::Timestamp",g.actor_id,c.closed_at AS "closed_at: jiff_sqlx::Timestamp",
COALESCE((SELECT string_agg(scope,' ' ORDER BY scope) FROM clacks.desktop_access_scopes WHERE access_digest=t.access_digest),'') AS "scope!",
ARRAY(SELECT resource FROM clacks.desktop_access_resources WHERE access_digest=t.access_digest ORDER BY ordinal) AS "resources!"
FROM clacks.desktop_access_tokens t JOIN clacks.desktop_grants g USING (account_id,family_id)
LEFT JOIN clacks.desktop_clients c ON c.account_id=g.account_id AND c.actor_id=g.actor_id
WHERE t.access_digest=$1 AND t.account_id=$2
