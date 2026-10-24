SELECT original_id,local_payload,shared_observation,reason,unknown_reason,reference_kind,reference_id,actor_id FROM local_review WHERE account = ? AND kind = ? AND record_id = ?
