SELECT account_id, provider, material_id, material_revision, encrypted_envelope FROM clacks.directory_material WHERE account_id = $1 AND provider = $2
