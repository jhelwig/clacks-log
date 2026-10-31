INSERT INTO clacks.accepted_head (account, kind, record_id, envelope_id) VALUES ($1, $2, $3, $4)
ON CONFLICT (account, kind, record_id) DO UPDATE SET envelope_id = EXCLUDED.envelope_id
