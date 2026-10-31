SELECT a.envelope_id FROM clacks.accepted_record a JOIN clacks.version_stamp s ON s.stamp_id = a.stamp_id
WHERE a.account = $1 AND a.kind = $2 AND a.record_id = $3 AND s.timeline_id = $4 AND s.revision = $5 AND s.baseline_id = $6 ORDER BY a.envelope_id
