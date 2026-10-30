UPDATE clacks.account_encryption_keys SET wrapped_envelope = $3
WHERE account_id = $1 AND key_version = $2
