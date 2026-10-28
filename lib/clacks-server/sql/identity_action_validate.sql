SELECT l.identity_id FROM clacks.login_sessions l
JOIN clacks.login_identities a ON a.account_id = l.account_id AND a.identity_id = $3
WHERE l.account_id = $1 AND l.login_reference = $2 AND l.expires_at > CURRENT_TIMESTAMP
