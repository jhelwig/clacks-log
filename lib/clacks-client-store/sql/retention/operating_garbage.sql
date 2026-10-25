DELETE FROM operating_value
WHERE NOT EXISTS (SELECT 1 FROM operating_state s WHERE s.current_values = operating_value.operating_id)
