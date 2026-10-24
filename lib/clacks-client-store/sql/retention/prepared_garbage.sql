DELETE FROM prepared_change WHERE NOT EXISTS (SELECT 1 FROM pending_record p WHERE p.prepared_id = prepared_change.prepared_id)
