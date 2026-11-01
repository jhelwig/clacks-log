DELETE FROM baseline WHERE account = ?1 AND baseline_row IN (
    SELECT b.baseline_row FROM baseline b WHERE b.account = ?1
      AND NOT EXISTS (SELECT 1 FROM version_stamp s WHERE s.account = b.account AND s.baseline_id = b.baseline_id)
      AND NOT EXISTS (SELECT 1 FROM checkpoint c WHERE c.account = b.account AND c.baseline_id = b.baseline_id)
      AND NOT EXISTS (SELECT 1 FROM witness_baseline w WHERE w.baseline_row = b.baseline_row)
    ORDER BY b.baseline_row LIMIT ?2
)
