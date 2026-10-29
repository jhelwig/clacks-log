UPDATE clacks.login_sessions
SET expires_at = CURRENT_TIMESTAMP + $2::bigint * INTERVAL '1 second'
WHERE login_reference = $1 AND expires_at > CURRENT_TIMESTAMP
RETURNING account_id, identity_id
