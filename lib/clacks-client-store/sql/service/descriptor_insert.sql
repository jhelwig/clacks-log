INSERT INTO destination_descriptor (payload_id, account, destination_id, collection, lotw_account, qrz_book, description) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING descriptor_id AS "descriptor_id!: i64"
