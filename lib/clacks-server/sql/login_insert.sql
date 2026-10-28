INSERT INTO clacks.login_sessions (login_reference, account_id, identity_id, expires_at)
VALUES ($1, $2, $3, CURRENT_TIMESTAMP + $4::bigint * INTERVAL '1 second')
