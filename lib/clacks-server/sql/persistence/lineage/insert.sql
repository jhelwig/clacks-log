INSERT INTO clacks.lineage (account, initial_timeline) VALUES ($1, $2) RETURNING lineage_id AS "lineage_id!: i64"
