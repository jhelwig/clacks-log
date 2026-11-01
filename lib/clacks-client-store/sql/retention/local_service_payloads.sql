SELECT s.payload_id, s.kind, s.record_id FROM service_support s
WHERE s.account = ?1 AND (
    EXISTS (SELECT 1 FROM original_basis o WHERE o.account = s.account AND o.payload_id = s.payload_id)
    OR EXISTS (SELECT 1 FROM pending_record p WHERE p.account = s.account AND p.working_payload = s.payload_id)
    OR EXISTS (SELECT 1 FROM prepared_change p WHERE p.account = s.account AND p.payload_id = s.payload_id)
    OR EXISTS (SELECT 1 FROM local_review r WHERE r.account = s.account AND r.local_payload = s.payload_id)
)
ORDER BY s.payload_id
