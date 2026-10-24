INSERT INTO export_expression (payload_id, kind, text, side, child, pattern, capture_name) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING expression_id AS "expression_id!: i64"
