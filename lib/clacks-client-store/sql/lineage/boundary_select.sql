SELECT ordinal, timeline_id, revision, kind, coverage_id FROM timeline_boundary WHERE lineage_id = ? AND account = ? ORDER BY ordinal
