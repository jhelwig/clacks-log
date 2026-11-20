SELECT record_id AS "record_id!", own_call AS "own_call!", start AS "start!",
       frequency, band, mode_kind AS "mode_kind!", mode_label AS "mode_label!", mode_parent
FROM local_qso
WHERE account = ? AND worked_call = ? AND start BETWEEN ? AND ?
ORDER BY start, record_id
