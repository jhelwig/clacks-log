SELECT account_id FROM clacks.account_encryption_keys WHERE wrapping_key_id = $1
ORDER BY account_id LIMIT 1
