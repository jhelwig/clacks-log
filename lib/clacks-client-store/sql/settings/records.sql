SELECT kind, record_id FROM client_record
WHERE account = ? AND kind IN ('profile', 'equipment', 'group')
ORDER BY kind, record_id
