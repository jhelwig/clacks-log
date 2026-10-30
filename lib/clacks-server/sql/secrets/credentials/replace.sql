UPDATE clacks.service_credentials SET material_revision = $3, state = 'present',
    payload_format = $4, key_version = $5, nonce = $6, ciphertext = $7, changed_at = $8
WHERE account_id = $1 AND credential_id = $2
