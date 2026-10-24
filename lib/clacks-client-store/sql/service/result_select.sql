SELECT destination_id, scope, batch_id, outcome, outcome_detail, phase, at_seconds, at_nanos, diagnostic, external_id, external_attempt FROM service_result WHERE payload_id = ? AND account = ?
