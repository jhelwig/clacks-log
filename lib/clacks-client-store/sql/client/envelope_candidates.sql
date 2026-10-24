SELECT a.envelope_id FROM accepted_record a JOIN version_stamp s ON s.stamp_id = a.stamp_id
WHERE a.account = ? AND a.kind = ? AND a.record_id = ? AND s.timeline_id = ? AND s.revision = ? AND s.baseline_id = ? ORDER BY a.envelope_id
