INSERT INTO clacks.record_vector (account, kind, record_id, body) VALUES ($1, $2, $3, $4) RETURNING vector_id AS "vector_id!: i64"
