DELETE FROM operating_value WHERE account = ?1 AND operating_id IN (
    SELECT v.operating_id FROM operating_value v WHERE v.account = ?1
      AND NOT EXISTS (SELECT 1 FROM operating_state s WHERE s.account = v.account AND s.current_values = v.operating_id)
    ORDER BY v.operating_id LIMIT ?2
)
