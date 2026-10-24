DELETE FROM operating_value
WHERE NOT EXISTS (SELECT 1 FROM operating_state s WHERE s.current_values = operating_value.operating_id OR s.loaded_values = operating_value.operating_id)
