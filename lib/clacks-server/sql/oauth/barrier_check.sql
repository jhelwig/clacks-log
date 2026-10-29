SELECT EXISTS (SELECT 1 FROM clacks.oauth_revocations
WHERE (scope = 'family' AND target = $1) OR
(scope = 'client' AND target = $2 AND recorded_at >= $3)) AS "revoked!"
