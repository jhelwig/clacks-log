SELECT qso_id, projection_id, phase, at_seconds, at_nanos, batch_id FROM clacks.service_attempt WHERE payload_id = $1 AND account = $2
