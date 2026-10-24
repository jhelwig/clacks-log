DELETE FROM record_observation
WHERE NOT EXISTS (SELECT 1 FROM client_record c WHERE c.latest_observation = record_observation.observation_id)
AND NOT EXISTS (SELECT 1 FROM local_review r WHERE r.shared_observation = record_observation.observation_id)
