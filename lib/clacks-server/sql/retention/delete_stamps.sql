DELETE FROM clacks.version_stamp s WHERE s.account = $1 AND s.stamp_id = ANY(ARRAY(
    SELECT s.stamp_id FROM clacks.version_stamp s WHERE s.account = $1 AND NOT (s.stamp_id = ANY($2::bigint[]))
      AND NOT EXISTS (SELECT 1 FROM clacks.accepted_record a WHERE a.account = s.account AND a.stamp_id = s.stamp_id)
      AND NOT EXISTS (SELECT 1 FROM clacks.installation_journal j WHERE j.account = s.account AND j.stamp_id = s.stamp_id)
      AND NOT EXISTS (SELECT 1 FROM clacks.service_support r WHERE r.account = s.account AND r.acquisition_stamp = s.stamp_id)
      AND NOT EXISTS (SELECT 1 FROM clacks.support_requirement r WHERE r.account = s.account AND r.stamp_id = s.stamp_id)
      AND NOT EXISTS (SELECT 1 FROM clacks.coupled_member c WHERE c.account = s.account AND c.stamp_id = s.stamp_id)
    ORDER BY s.stamp_id LIMIT $3
))
