SELECT external_id, qso_id FROM clacks.service_suppression WHERE payload_id = $1 AND account = $2
