SELECT kind, record_id, envelope_id FROM clacks.accepted_head WHERE account = $1 ORDER BY kind, record_id
