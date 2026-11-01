DELETE FROM version_condition WHERE account = ?1 AND condition_id IN (
    SELECT c.condition_id FROM version_condition c WHERE c.account = ?1
      AND NOT EXISTS (SELECT 1 FROM prepared_change p WHERE p.account = c.account AND p.condition_id = c.condition_id)
      AND NOT EXISTS (SELECT 1 FROM prepared_dependency d WHERE d.condition_id = c.condition_id)
    ORDER BY c.condition_id LIMIT ?2
)
