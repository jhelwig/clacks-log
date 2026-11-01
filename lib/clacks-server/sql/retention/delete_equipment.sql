DELETE FROM clacks.equipment_snapshot e WHERE e.account = $1 AND e.equipment_id IN (
    SELECT e.equipment_id FROM clacks.equipment_snapshot e WHERE e.account = $1
      AND NOT EXISTS (SELECT 1 FROM clacks.qso_payload q WHERE q.account = e.account AND q.equipment_id = e.equipment_id)
      AND NOT EXISTS (SELECT 1 FROM clacks.equipment_payload p WHERE p.account = e.account AND p.snapshot_id = e.equipment_id)
    ORDER BY e.equipment_id LIMIT $2
)
