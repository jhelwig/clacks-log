SELECT b.association_id, b.envelope_id
FROM clacks.external_key e LEFT JOIN clacks.current_external_binding b ON b.account = e.account AND b.external_id = e.external_id
WHERE e.account = $1 AND e.destination_id = $2 AND e.canonical_key = $3
