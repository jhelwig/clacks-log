DELETE FROM basis_witness WHERE account = ?1 AND witness_id IN (
    SELECT w.witness_id FROM basis_witness w WHERE w.account = ?1
      AND NOT EXISTS (SELECT 1 FROM pending_record p WHERE p.account = w.account AND p.witness_id = w.witness_id)
    ORDER BY w.witness_id LIMIT ?2
)
