SELECT account_id, provider, material_id, material_revision, encrypted_envelope
FROM clacks.directory_material WHERE account_id = $1 AND account_key_version = $2
ORDER BY provider LIMIT $3
