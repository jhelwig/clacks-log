DELETE FROM coverage WHERE account = ?1 AND coverage_id IN (
    SELECT c.coverage_id FROM coverage c WHERE c.account = ?1
      AND NOT EXISTS (SELECT 1 FROM checkpoint p WHERE p.account = c.account AND p.coverage_id = c.coverage_id)
      AND NOT EXISTS (SELECT 1 FROM baseline b WHERE b.account = c.account AND b.coverage_id = c.coverage_id)
      AND NOT EXISTS (SELECT 1 FROM timeline_boundary t WHERE t.account = c.account AND t.coverage_id = c.coverage_id)
    ORDER BY c.coverage_id LIMIT ?2
)
