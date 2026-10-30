SELECT account_id, credential_id, purpose, material_revision, state, payload_format,
       key_version, nonce, ciphertext, created_at, changed_at
FROM clacks.service_credentials WHERE account_id = $1 AND credential_id = $2
