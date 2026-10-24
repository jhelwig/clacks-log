INSERT INTO transfer_window (account, source_checkpoint, mode, after_coverage) VALUES (?, ?, ?, ?) RETURNING window_id AS "window_id!: i64"
