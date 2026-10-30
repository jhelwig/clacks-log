INSERT INTO clacks.service_credentials
(account_id, credential_id, purpose, material_revision, state, payload_format, key_version, nonce, ciphertext, created_at, changed_at)
VALUES ($1, $2, $3, $4, 'present', $5, $6, $7, $8, $9, $9)
