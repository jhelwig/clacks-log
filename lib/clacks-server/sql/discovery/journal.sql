SELECT j.timeline_id, j.revision, j.kind, j.record_id, j.stamp_id,
       h.envelope_id
FROM clacks.installation_journal j
JOIN unnest($2::text[], $3::bigint[], $4::bigint[]) AS bounds(timeline, after_revision, through_revision)
  ON j.timeline_id = bounds.timeline AND j.revision > bounds.after_revision AND j.revision <= bounds.through_revision
JOIN clacks.accepted_head h USING (account, kind, record_id)
WHERE j.account = $1 AND (j.timeline_id, j.revision) > ($5, $6)
ORDER BY j.timeline_id, j.revision LIMIT $7
