SELECT account_id, identity_id FROM clacks.login_identities
WHERE issuer = $1 AND subject = $2
