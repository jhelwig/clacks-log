DELETE FROM lineage
WHERE NOT EXISTS (SELECT 1 FROM current_lineage c WHERE c.lineage_id = lineage.lineage_id)
AND NOT EXISTS (SELECT 1 FROM baseline b WHERE b.lineage_id = lineage.lineage_id)
AND NOT EXISTS (SELECT 1 FROM basis_witness w WHERE w.lineage_id = lineage.lineage_id)
