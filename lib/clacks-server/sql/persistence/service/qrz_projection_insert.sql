INSERT INTO clacks.submission_projection (payload_id, account, destination_id, service, contact_fields) VALUES ($1, $2, $3, 'qrz', $4) RETURNING projection_id AS "projection_id!: i64"
