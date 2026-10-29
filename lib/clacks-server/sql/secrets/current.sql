SELECT key_version, wrapping_key_id, wrapping_nonce, wrapped_key
FROM clacks.account_encryption_keys WHERE account_id = $1 AND active
