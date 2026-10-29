INSERT INTO clacks.account_encryption_keys
(account_id, key_version, wrapping_key_id, wrapping_nonce, wrapped_key)
VALUES ($1, $2, $3, $4, $5)
