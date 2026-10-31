SELECT DISTINCT operation_id FROM clacks.coupled_member
WHERE account = $1 AND kind = $2 AND record_id = $3 ORDER BY operation_id
