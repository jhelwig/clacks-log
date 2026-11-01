SELECT j.installation_order, j.deleted, j.kind, j.record_id, j.stamp_id,
       h.envelope_id
FROM clacks.installation_journal j
JOIN unnest($2::text[], $3::bigint[], $4::bigint[]) AS bounds(timeline, after_revision, through_revision)
  ON j.timeline_id = bounds.timeline AND j.revision > bounds.after_revision AND j.revision <= bounds.through_revision
JOIN clacks.accepted_head h USING (account, kind, record_id)
WHERE j.account = $1 AND j.installation_order > $5
ORDER BY j.installation_order LIMIT $6
