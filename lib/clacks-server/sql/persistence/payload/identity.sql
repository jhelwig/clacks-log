INSERT INTO clacks.record_identity (account, kind, record_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING
