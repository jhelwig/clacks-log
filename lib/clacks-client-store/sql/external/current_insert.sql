INSERT INTO current_external_binding (account,external_id,association_id,envelope_id,payload_id)
SELECT c.account,s.external_id,c.record_id,o.envelope_id,a.payload_id
FROM client_record c JOIN record_observation o ON o.observation_id = c.latest_observation
JOIN accepted_record a ON a.envelope_id = o.envelope_id
JOIN service_association s ON s.payload_id = a.payload_id
WHERE c.account = ? AND c.kind = 'association' AND c.record_id = ?
