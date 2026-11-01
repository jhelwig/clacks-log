DELETE FROM record_vector WHERE account = ?1 AND vector_id IN (
    SELECT v.vector_id FROM record_vector v WHERE v.account = ?1
      AND NOT EXISTS (SELECT 1 FROM version_stamp s WHERE s.account = v.account AND s.vector_id = v.vector_id)
      AND NOT EXISTS (SELECT 1 FROM original_basis o WHERE o.account = v.account AND o.vector_id = v.vector_id)
      AND NOT EXISTS (SELECT 1 FROM prepared_change p WHERE p.account = v.account AND p.vector_id = v.vector_id)
    ORDER BY v.vector_id LIMIT ?2
)
