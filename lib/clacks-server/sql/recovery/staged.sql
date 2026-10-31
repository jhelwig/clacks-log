SELECT kind, record_id, envelope_id FROM clacks.staged_accepted WHERE account = $1 ORDER BY kind, record_id, envelope_id
