UPDATE clacks.service_credentials SET material_revision = $3, state = 'removed',
    payload_format = NULL, key_version = NULL, nonce = NULL, ciphertext = NULL, changed_at = $4
WHERE account_id = $1 AND credential_id = $2
