DELETE FROM clacks.payload p WHERE p.account = $1 AND p.payload_id IN (
    SELECT p.payload_id FROM clacks.payload p WHERE p.account = $1
      AND NOT EXISTS (SELECT 1 FROM clacks.accepted_record a WHERE a.account = p.account AND a.payload_id = p.payload_id)
    ORDER BY p.payload_id LIMIT $2
)
