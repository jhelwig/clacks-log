INSERT INTO service_query (payload_id, account, destination_id, stream, own_call, since) VALUES (?, ?, ?, ?, ?, ?) RETURNING query_id AS "query_id!: i64"
