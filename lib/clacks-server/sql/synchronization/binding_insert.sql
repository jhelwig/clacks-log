INSERT INTO clacks.current_external_binding (account, external_id, association_id, envelope_id)
SELECT h.account, s.external_id, h.record_id, a.envelope_id
FROM clacks.accepted_head h JOIN clacks.accepted_record a ON a.envelope_id = h.envelope_id
JOIN clacks.service_association s ON s.payload_id = a.payload_id
WHERE h.account = $1 AND h.kind = 'association' AND h.record_id = $2
