SELECT q.record_id FROM local_qso q WHERE q.account = ? AND EXISTS (SELECT 1 FROM qso_tag t WHERE t.payload_id = q.payload_id AND t.tag_id = ?) ORDER BY q.record_id
