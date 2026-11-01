DELETE FROM clacks.record_vector v WHERE v.account = $1 AND v.vector_id IN (
    SELECT v.vector_id FROM clacks.record_vector v WHERE v.account = $1
      AND NOT EXISTS (SELECT 1 FROM clacks.version_stamp s WHERE s.account = v.account AND s.vector_id = v.vector_id)
    ORDER BY v.vector_id LIMIT $2
)
