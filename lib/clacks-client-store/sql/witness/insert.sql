INSERT INTO basis_witness (account, kind, record_id, original_id, observed_stamp, checkpoint_id, relation, lineage_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING witness_id AS "witness_id!: i64"
