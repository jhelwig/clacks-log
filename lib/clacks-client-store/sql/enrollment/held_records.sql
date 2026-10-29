SELECT kind, record_id FROM client_record
WHERE account=? AND authorization_hold IS NOT NULL AND pending_id IS NOT NULL
ORDER BY kind, record_id
