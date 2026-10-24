SELECT DISTINCT d.destination_id,d.collection,d.lotw_account,d.qrz_book,d.description
FROM client_record c JOIN record_observation o ON o.observation_id = c.latest_observation
JOIN accepted_record a ON a.envelope_id = o.envelope_id
JOIN destination_descriptor d ON d.payload_id = a.payload_id
WHERE c.account = ?
ORDER BY d.destination_id
