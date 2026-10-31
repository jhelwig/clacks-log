SELECT payload_id, stamp_id FROM clacks.accepted_record WHERE envelope_id = $1 AND account = $2 AND kind = $3 AND record_id = $4
