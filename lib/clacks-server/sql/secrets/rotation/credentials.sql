SELECT account_id, credential_id, purpose, material_revision, state, payload_format,
       key_version, nonce, ciphertext, created_at, changed_at
FROM clacks.service_credentials WHERE account_id = $1 AND key_version = $2
ORDER BY credential_id LIMIT $3
