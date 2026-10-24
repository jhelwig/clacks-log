DELETE FROM coverage
WHERE NOT EXISTS (SELECT 1 FROM checkpoint c WHERE c.coverage_id = coverage.coverage_id)
AND NOT EXISTS (SELECT 1 FROM baseline b WHERE b.coverage_id = coverage.coverage_id)
AND NOT EXISTS (SELECT 1 FROM account_metadata a WHERE a.coverage_id = coverage.coverage_id)
AND NOT EXISTS (SELECT 1 FROM timeline_boundary b WHERE b.coverage_id = coverage.coverage_id)
AND NOT EXISTS (SELECT 1 FROM transfer_window w WHERE w.after_coverage = coverage.coverage_id)
AND NOT EXISTS (SELECT 1 FROM window_boundary b WHERE b.coverage_id = coverage.coverage_id)
