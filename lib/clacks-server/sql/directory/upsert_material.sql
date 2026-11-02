INSERT INTO clacks.directory_material(account_id, provider, material_id, material_revision, encrypted_envelope) VALUES ($1, $2, $3, $4, $5)
ON CONFLICT(account_id, provider) DO UPDATE SET material_id = EXCLUDED.material_id, material_revision = EXCLUDED.material_revision, encrypted_envelope = EXCLUDED.encrypted_envelope
