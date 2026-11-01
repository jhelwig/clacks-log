DELETE FROM accepted_record WHERE account = ?1 AND envelope_id IN (
    SELECT envelope_id FROM accepted_record WHERE account = ?1
      AND envelope_id NOT IN (SELECT CAST(value AS INTEGER) FROM json_each(?2)) ORDER BY envelope_id LIMIT ?3
)
