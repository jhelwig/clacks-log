WITH RECURSIVE selected_tag(value, rest) AS (
    SELECT '', ?1 || ','
    UNION ALL SELECT substr(rest,1,instr(rest,',')-1),substr(rest,instr(rest,',')+1) FROM selected_tag WHERE rest != ''
)
SELECT q.record_id,q.payload_id,q.pending AS "pending!: bool"
FROM local_qso q
WHERE q.account = ?2
AND (?1 = '' OR EXISTS (SELECT 1 FROM qso_tag t JOIN selected_tag s ON s.value = t.tag_id WHERE t.payload_id = q.payload_id))
AND (?3 IS NULL OR q.worked_call LIKE ?3 ESCAPE '!')
AND (?4 IS NULL OR q.start >= ?4) AND (?5 IS NULL OR q.start <= ?5)
AND (?6 IS NULL OR q.band = ?6) AND (?7 IS NULL OR q.mode_label = ?7)
AND (?8 IS NULL OR q.own_call = ?8)
AND (?9 IS NULL OR q.start > ?9 OR (q.start = ?9 AND q.record_id > ?10))
ORDER BY q.start,q.record_id LIMIT ?11
