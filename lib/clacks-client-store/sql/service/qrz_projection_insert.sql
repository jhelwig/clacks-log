INSERT INTO submission_projection (payload_id, account, destination_id, service, contact_map) VALUES (?, ?, ?, 'qrz', ?) RETURNING projection_id AS "projection_id!: i64"
