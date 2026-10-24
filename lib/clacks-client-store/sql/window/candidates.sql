SELECT w.window_id AS "window_id!: i64" FROM transfer_window w JOIN checkpoint c ON c.checkpoint_id = w.source_checkpoint WHERE w.account = ? AND c.baseline_id = ? AND w.mode = ? ORDER BY w.window_id
