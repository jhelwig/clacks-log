DELETE FROM record_observation WHERE account = ?1 AND observation_id IN (
    SELECT o.observation_id FROM record_observation o WHERE o.account = ?1
      AND NOT EXISTS (SELECT 1 FROM client_record c WHERE c.account = o.account AND c.latest_observation = o.observation_id)
      AND NOT EXISTS (SELECT 1 FROM local_review r WHERE r.account = o.account AND r.shared_observation = o.observation_id)
    ORDER BY o.observation_id LIMIT ?2
)
