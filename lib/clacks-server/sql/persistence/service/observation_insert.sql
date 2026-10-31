INSERT INTO clacks.service_observation (payload_id, account, destination_id, identity, external_id, incomplete_service, available_fields, contact_report, own_report, counterpart_report, qso_order, confirmation_order, acquisition_id, query_id, observed_seconds, observed_nanos)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
