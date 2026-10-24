DELETE FROM equipment_snapshot
WHERE NOT EXISTS (SELECT 1 FROM qso_payload q WHERE q.equipment_id = equipment_snapshot.equipment_id)
AND NOT EXISTS (SELECT 1 FROM equipment_payload e WHERE e.snapshot_id = equipment_snapshot.equipment_id)
AND NOT EXISTS (SELECT 1 FROM operating_value o WHERE o.equipment_id = equipment_snapshot.equipment_id)
