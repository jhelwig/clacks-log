DELETE FROM basis_witness WHERE NOT EXISTS (SELECT 1 FROM pending_record p WHERE p.witness_id = basis_witness.witness_id)
