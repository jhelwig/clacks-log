INSERT INTO version_stamp (account, kind, record_id, timeline_id, revision, baseline_id, vector_id) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING stamp_id AS "stamp_id!: i64"
