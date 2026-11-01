DELETE FROM pending_record WHERE account = ?1 AND pending_id IN (
    SELECT p.pending_id FROM pending_record p WHERE p.account = ?1
      AND NOT EXISTS (SELECT 1 FROM client_record c WHERE c.account = p.account AND c.pending_id = p.pending_id)
    ORDER BY p.pending_id LIMIT ?2
)
