DELETE FROM record_vector
WHERE NOT EXISTS (SELECT 1 FROM version_stamp s WHERE s.vector_id = record_vector.vector_id)
AND NOT EXISTS (SELECT 1 FROM original_basis o WHERE o.vector_id = record_vector.vector_id)
AND NOT EXISTS (SELECT 1 FROM prepared_change p WHERE p.vector_id = record_vector.vector_id)
