SELECT key_version, wrapped_envelope FROM clacks.account_encryption_keys
WHERE account_id = $1 AND active
