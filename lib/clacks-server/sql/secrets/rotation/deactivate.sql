UPDATE clacks.account_encryption_keys SET active = false WHERE account_id = $1 AND key_version = $2 AND active
