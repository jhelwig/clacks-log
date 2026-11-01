DELETE FROM original_basis WHERE account = ?1 AND original_id IN (
    SELECT o.original_id FROM original_basis o WHERE o.account = ?1
      AND NOT EXISTS (SELECT 1 FROM pending_record p WHERE p.account = o.account AND p.original_id = o.original_id)
      AND NOT EXISTS (SELECT 1 FROM basis_witness w WHERE w.account = o.account AND w.original_id = o.original_id)
      AND NOT EXISTS (SELECT 1 FROM local_review r WHERE r.account = o.account AND r.original_id = o.original_id)
    ORDER BY o.original_id LIMIT ?2
)
