DELETE FROM local_counter WHERE account = ?1 AND rowid IN (
    SELECT c.rowid FROM local_counter c WHERE c.account = ?1
      AND c.actor_id <> (SELECT actor FROM store_account WHERE account = ?1)
      AND NOT EXISTS (SELECT 1 FROM client_record r WHERE r.account = c.account AND r.kind = c.kind AND r.record_id = c.record_id AND (r.pending_id IS NOT NULL OR r.authorization_hold IS NOT NULL))
      AND NOT EXISTS (SELECT 1 FROM prepared_change p WHERE p.account = c.account AND p.kind = c.kind AND p.record_id = c.record_id)
    ORDER BY c.rowid LIMIT ?2
)
