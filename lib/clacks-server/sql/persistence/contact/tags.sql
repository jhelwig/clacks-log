SELECT tag_id FROM clacks.qso_tag WHERE payload_id = $1 AND account = $2 ORDER BY tag_id
