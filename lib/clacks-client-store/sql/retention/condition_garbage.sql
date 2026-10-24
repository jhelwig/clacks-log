DELETE FROM version_condition
WHERE NOT EXISTS (SELECT 1 FROM prepared_change p WHERE p.condition_id = version_condition.condition_id)
AND NOT EXISTS (SELECT 1 FROM prepared_dependency d WHERE d.condition_id = version_condition.condition_id)
