DELETE FROM version_stamp
WHERE NOT EXISTS (SELECT 1 FROM accepted_record a WHERE a.stamp_id = version_stamp.stamp_id)
AND NOT EXISTS (SELECT 1 FROM original_basis o WHERE o.stamp_id = version_stamp.stamp_id)
AND NOT EXISTS (SELECT 1 FROM version_condition c WHERE c.stamp_id = version_stamp.stamp_id)
AND NOT EXISTS (SELECT 1 FROM basis_witness w WHERE w.observed_stamp = version_stamp.stamp_id)
AND NOT EXISTS (SELECT 1 FROM window_requirement r WHERE r.stamp_id = version_stamp.stamp_id)
AND NOT EXISTS (SELECT 1 FROM window_unit_member m WHERE m.stamp_id = version_stamp.stamp_id)
AND NOT EXISTS (SELECT 1 FROM service_support s WHERE s.acquisition_stamp = version_stamp.stamp_id)
AND NOT EXISTS (SELECT 1 FROM support_requirement r WHERE r.stamp_id = version_stamp.stamp_id)
