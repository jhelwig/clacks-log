DELETE FROM payload
WHERE NOT EXISTS (SELECT 1 FROM accepted_record a WHERE a.payload_id = payload.payload_id)
AND NOT EXISTS (SELECT 1 FROM pending_record p WHERE p.working_payload = payload.payload_id)
AND NOT EXISTS (SELECT 1 FROM prepared_change p WHERE p.payload_id = payload.payload_id)
AND NOT EXISTS (SELECT 1 FROM original_basis o WHERE o.payload_id = payload.payload_id)
AND NOT EXISTS (SELECT 1 FROM local_review r WHERE r.local_payload = payload.payload_id)
AND NOT EXISTS (SELECT 1 FROM operating_state o WHERE o.loaded_profile = payload.payload_id OR o.loaded_equipment = payload.payload_id)
