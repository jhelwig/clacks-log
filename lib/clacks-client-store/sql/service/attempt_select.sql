SELECT qso_id, projection_id, phase, at_seconds, at_nanos, batch_id FROM service_attempt WHERE payload_id = ? AND account = ?
