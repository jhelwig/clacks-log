-- Journal obligations survive collection of historical native bodies.
ALTER TABLE clacks.installation_journal ADD COLUMN deleted BOOLEAN;
UPDATE clacks.installation_journal j
SET deleted = p.deleted
FROM clacks.accepted_record a
JOIN clacks.version_stamp s USING (stamp_id)
JOIN clacks.payload p ON p.payload_id = a.payload_id
JOIN clacks.record_vector v ON v.vector_id = s.vector_id
WHERE j.account = a.account AND j.kind = a.kind AND j.record_id = a.record_id
  AND EXISTS (
      SELECT 1 FROM clacks.version_stamp original
      JOIN clacks.record_vector original_vector ON original_vector.vector_id = original.vector_id
      WHERE original.stamp_id = j.stamp_id
        AND original.baseline_id = s.baseline_id
        AND original.timeline_id = s.timeline_id AND original.revision = s.revision
        AND original_vector.body = v.body
  );
ALTER TABLE clacks.installation_journal ALTER COLUMN deleted SET NOT NULL;

-- Bootstrap existing rows in stable per-timeline revision order. Cross-timeline
-- ordering is a retention preference, never reconstructed causal evidence.
ALTER TABLE clacks.installation_journal ADD COLUMN installation_order BIGINT;
WITH ordered AS (
    SELECT account, timeline_id, revision, kind, record_id,
           row_number() OVER (ORDER BY account, timeline_id, revision, kind, record_id) AS ordinal
    FROM clacks.installation_journal
)
UPDATE clacks.installation_journal j SET installation_order = o.ordinal
FROM ordered o
WHERE (j.account, j.timeline_id, j.revision, j.kind, j.record_id)
    = (o.account, o.timeline_id, o.revision, o.kind, o.record_id);
ALTER TABLE clacks.installation_journal ALTER COLUMN installation_order SET NOT NULL;
ALTER TABLE clacks.installation_journal ALTER COLUMN installation_order ADD GENERATED ALWAYS AS IDENTITY;
SELECT setval(pg_get_serial_sequence('clacks.installation_journal', 'installation_order'),
              coalesce(max(installation_order), 0) + 1, false)
FROM clacks.installation_journal;
CREATE INDEX installation_journal_append ON clacks.installation_journal(account, installation_order);
