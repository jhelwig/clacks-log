INSERT INTO account_metadata (account, head_timeline, head_revision, baseline_id, coverage_id, membership_generation) VALUES (?, ?, ?, ?, ?, ?)
ON CONFLICT (account) DO UPDATE SET head_timeline = excluded.head_timeline, head_revision = excluded.head_revision, baseline_id = excluded.baseline_id, coverage_id = excluded.coverage_id, membership_generation = excluded.membership_generation
