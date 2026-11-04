DELETE FROM clacks.record_vector v
WHERE v.account = $1 AND v.vector_id = ANY(ARRAY(
    SELECT vector_id FROM clacks.record_vector WHERE account = $1
    EXCEPT
    SELECT vector_id FROM clacks.version_stamp WHERE account = $1
    ORDER BY vector_id LIMIT $2
))
