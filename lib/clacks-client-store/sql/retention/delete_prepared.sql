DELETE FROM prepared_change WHERE account = ?1 AND prepared_id IN (
    SELECT p.prepared_id FROM prepared_change p WHERE p.account = ?1
      AND NOT EXISTS (SELECT 1 FROM pending_record r WHERE r.account = p.account AND r.prepared_id = p.prepared_id)
    ORDER BY p.prepared_id LIMIT ?2
)
