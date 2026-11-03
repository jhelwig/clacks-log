SELECT EXISTS (SELECT 1 FROM clacks.login_identities WHERE account_id = $1 AND identity_id = $2) AS "present!"
