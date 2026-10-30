SELECT key_version, active FROM clacks.account_encryption_keys
WHERE account_id = $1 ORDER BY key_version
