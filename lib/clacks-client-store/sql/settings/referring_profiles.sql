SELECT p.record_id FROM profile_payload p
JOIN local_working w ON w.account = p.account AND w.kind = 'profile'
    AND w.record_id = p.record_id AND w.payload_id = p.payload_id
JOIN payload root ON root.payload_id = p.payload_id
WHERE p.account = ? AND p.equipment_id = ? AND root.deleted = 0
ORDER BY p.record_id
