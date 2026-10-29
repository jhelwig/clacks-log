DELETE FROM clacks.oauth_replay_claims WHERE replay_id IN (SELECT replay_id FROM clacks.oauth_replay_claims WHERE expires_at <= $1 LIMIT 500)
