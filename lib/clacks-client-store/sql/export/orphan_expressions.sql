WITH RECURSIVE roots(expression_id) AS (
    SELECT catalog_lookup FROM group_payload WHERE payload_id = ?1 AND catalog_lookup IS NOT NULL
    UNION SELECT expression_id FROM export_field WHERE payload_id = ?1
    UNION SELECT expression_id FROM export_partition WHERE payload_id = ?1 AND expression_id IS NOT NULL
), reachable(expression_id) AS (
    SELECT expression_id FROM roots
    UNION SELECT e.child FROM export_expression e JOIN reachable r ON e.expression_id = r.expression_id
          WHERE e.payload_id = ?1 AND e.child IS NOT NULL
)
SELECT count(*) AS "orphans!: i64" FROM export_expression
WHERE payload_id = ?1 AND expression_id NOT IN (SELECT expression_id FROM reachable)
