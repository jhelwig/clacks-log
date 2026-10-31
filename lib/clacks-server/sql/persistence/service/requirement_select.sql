SELECT ordinal, requirement, account, kind, record_id, stamp_id, deleted FROM clacks.support_requirement WHERE payload_id = $1 ORDER BY ordinal
