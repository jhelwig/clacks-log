DELETE FROM checkpoint
WHERE NOT EXISTS (SELECT 1 FROM installed_checkpoint i WHERE i.checkpoint_id = checkpoint.checkpoint_id)
AND NOT EXISTS (SELECT 1 FROM client_membership m WHERE m.checkpoint_id = checkpoint.checkpoint_id)
AND NOT EXISTS (SELECT 1 FROM record_observation o WHERE o.checkpoint_id = checkpoint.checkpoint_id)
AND NOT EXISTS (SELECT 1 FROM original_basis o WHERE o.checkpoint_id = checkpoint.checkpoint_id)
AND NOT EXISTS (SELECT 1 FROM version_condition c WHERE c.checkpoint_id = checkpoint.checkpoint_id)
AND NOT EXISTS (SELECT 1 FROM basis_witness w WHERE w.checkpoint_id = checkpoint.checkpoint_id)
AND NOT EXISTS (SELECT 1 FROM transfer_window w WHERE w.source_checkpoint = checkpoint.checkpoint_id)
