SELECT external_id, qso_id, state FROM clacks.service_association WHERE payload_id = $1 AND account = $2
