DELETE FROM checkpoint WHERE account = ?1 AND checkpoint_id IN (
    SELECT c.checkpoint_id FROM checkpoint c WHERE c.account = ?1
      AND NOT EXISTS (SELECT 1 FROM installed_checkpoint i WHERE i.account = c.account AND i.checkpoint_id = c.checkpoint_id)
      AND NOT EXISTS (SELECT 1 FROM record_observation o WHERE o.account = c.account AND o.checkpoint_id = c.checkpoint_id)
      AND NOT EXISTS (SELECT 1 FROM original_basis o WHERE o.account = c.account AND o.checkpoint_id = c.checkpoint_id)
      AND NOT EXISTS (SELECT 1 FROM version_condition v WHERE v.account = c.account AND v.checkpoint_id = c.checkpoint_id)
      AND NOT EXISTS (SELECT 1 FROM basis_witness w WHERE w.account = c.account AND w.checkpoint_id = c.checkpoint_id)
    ORDER BY c.checkpoint_id LIMIT ?2
)
