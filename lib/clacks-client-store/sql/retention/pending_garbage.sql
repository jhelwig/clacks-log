DELETE FROM pending_record WHERE NOT EXISTS (SELECT 1 FROM client_record c WHERE c.pending_id = pending_record.pending_id)
