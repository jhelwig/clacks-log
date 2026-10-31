INSERT INTO staged_accepted(account, kind, record_id, envelope_id) VALUES (?, ?, ?, ?) ON CONFLICT DO NOTHING
