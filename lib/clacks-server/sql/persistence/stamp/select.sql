SELECT timeline_id, revision, baseline_id, vector_id FROM clacks.version_stamp WHERE stamp_id = $1 AND account = $2 AND kind = $3 AND record_id = $4
