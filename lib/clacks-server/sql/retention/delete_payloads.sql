DELETE FROM clacks.payload p
WHERE p.account = $1 AND p.payload_id = ANY(ARRAY(
    SELECT payload_id FROM clacks.payload WHERE account = $1
    EXCEPT
    SELECT payload_id FROM clacks.accepted_record WHERE account = $1
    ORDER BY payload_id LIMIT $2
))
