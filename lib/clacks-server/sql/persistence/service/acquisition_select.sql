SELECT query_id, phase, failure, started_seconds, started_nanos, finished_seconds, finished_nanos, complete FROM clacks.service_acquisition WHERE payload_id = $1 AND account = $2
