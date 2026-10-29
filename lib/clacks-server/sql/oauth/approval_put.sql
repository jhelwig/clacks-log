INSERT INTO clacks.desktop_code_approvals (code_digest, account_id, purpose, target_actor, label, expires_at)
SELECT code_digest, account_id, $3, $4, $5, expires_at FROM clacks.desktop_authorization_codes
WHERE code_digest = $1 AND account_id = $2 AND state = 'issued' AND expires_at > CURRENT_TIMESTAMP
