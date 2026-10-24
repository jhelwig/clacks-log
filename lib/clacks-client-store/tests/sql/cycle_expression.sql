UPDATE export_expression SET child = expression_id
WHERE expression_id = (SELECT expression_id FROM export_expression WHERE payload_id = ? AND kind = 'capture' ORDER BY expression_id DESC LIMIT 1)
