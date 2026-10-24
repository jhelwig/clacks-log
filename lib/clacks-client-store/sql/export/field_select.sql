SELECT ordinal, field, expression_id, condition, binding, side FROM export_field WHERE plan_id = ? AND payload_id = ? ORDER BY ordinal
