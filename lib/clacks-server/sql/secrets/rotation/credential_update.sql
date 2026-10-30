UPDATE clacks.service_credentials SET key_version = $3, nonce = $4, ciphertext = $5
WHERE account_id = $1 AND credential_id = $2
