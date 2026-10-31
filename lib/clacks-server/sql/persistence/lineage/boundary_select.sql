SELECT ordinal, timeline_id, revision, kind, coverage_id FROM clacks.timeline_boundary WHERE lineage_id = $1 AND account = $2 ORDER BY ordinal
