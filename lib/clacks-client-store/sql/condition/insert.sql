INSERT INTO version_condition (account, kind, record_id, variant, stamp_id, checkpoint_id) VALUES (?, ?, ?, ?, ?, ?) RETURNING condition_id AS "condition_id!: i64"
