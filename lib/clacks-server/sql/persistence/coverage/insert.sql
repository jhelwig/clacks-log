INSERT INTO clacks.coverage (account, body) VALUES ($1, $2) RETURNING coverage_id AS "coverage_id!: i64"
