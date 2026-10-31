INSERT INTO clacks.version_stamp (account, kind, record_id, timeline_id, revision, baseline_id, vector_id) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING stamp_id AS "stamp_id!: i64"
