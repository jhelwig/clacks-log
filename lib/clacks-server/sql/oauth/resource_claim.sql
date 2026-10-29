INSERT INTO clacks.resource_replay_claims (key_thumbprint, proof_id, expires_at)
VALUES ($1,$2,$3) ON CONFLICT DO NOTHING RETURNING proof_id
