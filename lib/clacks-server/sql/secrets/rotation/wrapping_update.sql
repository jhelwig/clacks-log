UPDATE clacks.account_encryption_keys SET wrapping_key_id = $3, wrapping_nonce = $4, wrapped_key = $5
WHERE account_id = $1 AND key_version = $2
