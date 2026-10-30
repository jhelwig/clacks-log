SELECT key_version FROM clacks.account_encryption_keys WHERE account_id = $1 AND wrapping_key_id = $2
ORDER BY key_version LIMIT $3
