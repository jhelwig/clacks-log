DELETE FROM version_stamp WHERE account = ?1 AND stamp_id IN (
    SELECT s.stamp_id FROM version_stamp s WHERE s.account = ?1
      AND NOT EXISTS (SELECT 1 FROM accepted_record a WHERE a.account = s.account AND a.stamp_id = s.stamp_id)
      AND NOT EXISTS (SELECT 1 FROM original_basis o WHERE o.account = s.account AND o.stamp_id = s.stamp_id)
      AND NOT EXISTS (SELECT 1 FROM version_condition c WHERE c.account = s.account AND c.stamp_id = s.stamp_id)
      AND NOT EXISTS (SELECT 1 FROM basis_witness w WHERE w.account = s.account AND w.observed_stamp = s.stamp_id)
      AND NOT EXISTS (SELECT 1 FROM service_support r WHERE r.account = s.account AND r.acquisition_stamp = s.stamp_id)
      AND NOT EXISTS (SELECT 1 FROM support_requirement r WHERE r.account = s.account AND r.stamp_id = s.stamp_id)
      AND NOT EXISTS (SELECT 1 FROM coupled_member c WHERE c.account = s.account AND c.stamp_id = s.stamp_id)
    ORDER BY s.stamp_id LIMIT ?2
)
