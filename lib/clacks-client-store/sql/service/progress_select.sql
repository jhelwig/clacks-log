SELECT ordinal, query_id, watermark FROM stream_progress WHERE payload_id = ? ORDER BY ordinal
