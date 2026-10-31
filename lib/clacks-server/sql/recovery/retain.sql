INSERT INTO clacks.retained_accepted(account, kind, record_id, envelope_id) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING
