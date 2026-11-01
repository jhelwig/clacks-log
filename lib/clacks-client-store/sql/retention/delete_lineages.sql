DELETE FROM lineage WHERE account = ?1 AND lineage_id IN (
    SELECT l.lineage_id FROM lineage l WHERE l.account = ?1
      AND NOT EXISTS (SELECT 1 FROM current_lineage c WHERE c.account = l.account AND c.lineage_id = l.lineage_id)
      AND NOT EXISTS (SELECT 1 FROM baseline b WHERE b.account = l.account AND b.lineage_id = l.lineage_id)
      AND NOT EXISTS (SELECT 1 FROM basis_witness w WHERE w.account = l.account AND w.lineage_id = l.lineage_id)
    ORDER BY l.lineage_id LIMIT ?2
)
