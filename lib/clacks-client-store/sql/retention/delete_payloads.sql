DELETE FROM payload WHERE account = ?1 AND payload_id IN (
    SELECT p.payload_id FROM payload p WHERE p.account = ?1
      AND NOT EXISTS (SELECT 1 FROM accepted_record a WHERE a.account = p.account AND a.payload_id = p.payload_id)
      AND NOT EXISTS (SELECT 1 FROM original_basis o WHERE o.account = p.account AND o.payload_id = p.payload_id)
      AND NOT EXISTS (SELECT 1 FROM pending_record r WHERE r.account = p.account AND r.working_payload = p.payload_id)
      AND NOT EXISTS (SELECT 1 FROM prepared_change r WHERE r.account = p.account AND r.payload_id = p.payload_id)
      AND NOT EXISTS (SELECT 1 FROM local_review r WHERE r.account = p.account AND r.local_payload = p.payload_id)
      AND NOT EXISTS (SELECT 1 FROM operating_state s WHERE s.account = p.account AND (s.loaded_profile = p.payload_id OR s.loaded_equipment = p.payload_id))
    ORDER BY p.payload_id LIMIT ?2
)
