INSERT INTO station_report (payload_id, name_state, name, position_state, position_kind, locator, latitude, longitude, grid_length, dxcc_state, dxcc, primary_state, primary_subdivision, secondary_state, secondary_subdivision, cq_state, cq_zone, itu_state, itu_zone, references_state)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING report_id AS "report_id!: i64"
