SELECT ordinal, query_id, watermark FROM clacks.stream_progress WHERE payload_id = $1 ORDER BY ordinal
