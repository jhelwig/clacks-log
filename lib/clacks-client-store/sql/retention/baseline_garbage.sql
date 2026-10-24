DELETE FROM baseline
WHERE NOT EXISTS (SELECT 1 FROM version_stamp s WHERE s.account = baseline.account AND s.baseline_id = baseline.baseline_id)
AND NOT EXISTS (SELECT 1 FROM checkpoint c WHERE c.account = baseline.account AND c.baseline_id = baseline.baseline_id)
AND NOT EXISTS (SELECT 1 FROM account_metadata a WHERE a.account = baseline.account AND a.baseline_id = baseline.baseline_id)
AND NOT EXISTS (SELECT 1 FROM window_baseline w WHERE w.baseline_row = baseline.baseline_row)
AND NOT EXISTS (SELECT 1 FROM witness_baseline w WHERE w.baseline_row = baseline.baseline_row)
