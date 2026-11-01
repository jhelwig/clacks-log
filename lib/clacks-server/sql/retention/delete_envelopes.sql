DELETE FROM clacks.accepted_record WHERE account = $1 AND envelope_id IN (
    SELECT envelope_id FROM clacks.accepted_record
    WHERE account = $1 AND NOT (envelope_id = ANY($2::bigint[])) ORDER BY envelope_id LIMIT $3
)
