DELETE FROM original_basis
WHERE NOT EXISTS (SELECT 1 FROM pending_record p WHERE p.original_id = original_basis.original_id)
AND NOT EXISTS (SELECT 1 FROM basis_witness w WHERE w.original_id = original_basis.original_id)
AND NOT EXISTS (SELECT 1 FROM local_review r WHERE r.original_id = original_basis.original_id)
