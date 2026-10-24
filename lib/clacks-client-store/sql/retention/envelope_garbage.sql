DELETE FROM accepted_record
WHERE kind NOT IN ('imported','attempt','result','observation')
AND NOT EXISTS (SELECT 1 FROM record_observation o WHERE o.envelope_id = accepted_record.envelope_id)
AND NOT EXISTS (SELECT 1 FROM original_basis o WHERE o.envelope_id = accepted_record.envelope_id)
AND NOT EXISTS (SELECT 1 FROM retained_accepted r WHERE r.envelope_id = accepted_record.envelope_id)
AND NOT EXISTS (SELECT 1 FROM review_retained r WHERE r.envelope_id = accepted_record.envelope_id)
AND NOT EXISTS (SELECT 1 FROM window_staged s WHERE s.envelope_id = accepted_record.envelope_id)
AND NOT EXISTS (SELECT 1 FROM current_external_binding b WHERE b.envelope_id = accepted_record.envelope_id)
AND NOT EXISTS (SELECT 1 FROM support_requirement r WHERE r.requirement = 'fact' AND r.account = accepted_record.account AND r.kind = accepted_record.kind AND r.record_id = accepted_record.record_id)
