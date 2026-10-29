DELETE FROM clacks.resource_replay_claims WHERE (key_thumbprint,proof_id) IN (SELECT key_thumbprint,proof_id FROM clacks.resource_replay_claims WHERE expires_at <= $1 LIMIT 500)
