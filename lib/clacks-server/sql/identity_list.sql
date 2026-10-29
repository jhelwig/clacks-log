SELECT identity_id, issuer, subject, display_name FROM clacks.login_identities
WHERE account_id = $1 ORDER BY created_at, identity_id
