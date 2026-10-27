-- A point's display locator is computed only when sorting the grid column.
-- Mixed-radix projection uses the native model's integer microdegree rules.
WITH RECURSIVE selected_ids(value, rest) AS (
 SELECT '', coalesce(?8, '') || ','
 UNION ALL SELECT substr(rest, 1, instr(rest, ',') - 1), substr(rest, instr(rest, ',') + 1)
 FROM selected_ids WHERE rest != ''
), point_cells AS (
 SELECT station_id, grid_length / 2 AS pairs,
 CASE grid_length WHEN 2 THEN 18 WHEN 4 THEN 180 WHEN 6 THEN 4320
 WHEN 8 THEN 43200 WHEN 10 THEN 1036800 WHEN 12 THEN 10368000 END AS cells,
 CASE WHEN longitude = 180000000 THEN 0 ELSE longitude + 180000000 END AS lon,
 latitude + 90000000 AS lat
 FROM recorded_location WHERE account = ?1 AND position_kind = 'point' AND ?4 = 'grid'
), point_grids(station_id, step, x, y, locator) AS (
 SELECT station_id, pairs, lon * cells / 360000000,
 min(lat * cells / 180000000, cells - 1), '' FROM point_cells
 UNION ALL
 SELECT station_id, step - 1,
 x / CASE WHEN step = 1 THEN 18 WHEN step % 2 = 0 THEN 10 ELSE 24 END,
 y / CASE WHEN step = 1 THEN 18 WHEN step % 2 = 0 THEN 10 ELSE 24 END,
 char(CASE WHEN step % 2 = 1 THEN 65 ELSE 48 END +
 x % CASE WHEN step = 1 THEN 18 WHEN step % 2 = 0 THEN 10 ELSE 24 END) ||
 char(CASE WHEN step % 2 = 1 THEN 65 ELSE 48 END +
 y % CASE WHEN step = 1 THEN 18 WHEN step % 2 = 0 THEN 10 ELSE 24 END) || locator
 FROM point_grids WHERE step > 0
), selected AS (
SELECT q.record_id, q.payload_id, q.pending, q.start, q.worked_call,
 q.band, q.mode_label, q.frequency, p.sent_report, p.received_report, coalesce(l.locator, g.locator) AS locator,
 CASE WHEN r.record_id IS NOT NULL THEN 2 WHEN q.pending THEN 1 ELSE 0 END AS status
FROM local_qso q
JOIN qso_payload p ON p.payload_id = q.payload_id
LEFT JOIN recorded_location l ON l.station_id = p.contacted_station
LEFT JOIN point_grids g ON g.station_id = l.station_id AND g.step = 0
LEFT JOIN local_review r ON r.account = q.account AND r.kind = 'qso' AND r.record_id = q.record_id
WHERE q.account = ?1
AND (?8 IS NULL OR q.record_id IN (SELECT value FROM selected_ids WHERE value != ''))
AND (?2 IS NULL OR EXISTS (SELECT 1 FROM qso_tag t WHERE t.payload_id = q.payload_id AND t.tag_id = ?2))
AND (NOT ?3 OR NOT EXISTS (SELECT 1 FROM qso_tag t WHERE t.payload_id = q.payload_id))
), ordered AS (
SELECT *, CASE ?4 WHEN 'call' THEN worked_call WHEN 'start' THEN start
 WHEN 'band' THEN band WHEN 'mode' THEN mode_label WHEN 'frequency' THEN frequency
 WHEN 'sent' THEN sent_report WHEN 'received' THEN received_report
 WHEN 'grid' THEN locator WHEN 'status' THEN status END AS sort_value
FROM selected
)
SELECT record_id AS "record_id!", payload_id AS "payload_id!", pending AS "pending!: bool"
FROM ordered
ORDER BY sort_value IS NULL, CASE WHEN NOT ?5 THEN sort_value END ASC,
 CASE WHEN ?5 THEN sort_value END DESC, record_id ASC
LIMIT ?6 OFFSET ?7
