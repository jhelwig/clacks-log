SELECT EXISTS(SELECT 1 FROM clacks.account_encryption_keys WHERE account_id = $1) AS "present!"
