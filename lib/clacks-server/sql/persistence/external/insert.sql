INSERT INTO clacks.external_key (account, destination_id, canonical_key) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING
