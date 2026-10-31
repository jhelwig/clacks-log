INSERT INTO clacks.service_query (payload_id, account, destination_id, stream, own_call, since, selected_fields) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING query_id AS "query_id!: i64"
