SELECT kind, record_id, stamp_id, deleted FROM clacks.coupled_member
WHERE account = $1 AND operation_id = $2 ORDER BY ordinal
