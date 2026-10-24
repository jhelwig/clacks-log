SELECT ordinal, expression_id, side, binding FROM export_partition WHERE plan_id = ? AND payload_id = ? ORDER BY ordinal
