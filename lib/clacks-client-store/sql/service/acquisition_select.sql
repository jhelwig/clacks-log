SELECT query_id, phase, failure, started_seconds, started_nanos, finished_seconds, finished_nanos, complete FROM service_acquisition WHERE payload_id = ? AND account = ?
