-- Select only this batch's evidence, then its transitive interpretation support.
WITH RECURSIVE
ids AS (SELECT value AS id FROM json_each(?2)),
attempts AS (
 SELECT w.record_id, a.payload_id FROM local_working w
 JOIN service_attempt a ON a.payload_id = w.payload_id
 JOIN payload p ON p.payload_id = w.payload_id
 WHERE w.account = ?1 AND p.deleted = 0 AND a.qso_id IN (SELECT id FROM ids)
), associations AS (
 SELECT w.record_id, a.payload_id, a.external_id FROM local_working w
 JOIN service_association a ON a.payload_id = w.payload_id
 JOIN payload p ON p.payload_id = w.payload_id
 WHERE w.account = ?1 AND p.deleted = 0 AND a.qso_id IN (SELECT id FROM ids)
), roots(kind, record_id) AS (
 SELECT 'qso', id FROM ids
 UNION
 SELECT w.kind, w.record_id FROM local_working w
 JOIN service_assertion a ON a.payload_id = w.payload_id
 JOIN payload p ON p.payload_id = w.payload_id
 WHERE w.account = ?1 AND p.deleted = 0 AND a.qso_id IN (SELECT id FROM ids)
 UNION SELECT 'attempt', record_id FROM attempts
 UNION SELECT 'association', record_id FROM associations
 UNION
 SELECT w.kind, w.record_id FROM local_working w
 JOIN result_member r ON r.payload_id = w.payload_id
 JOIN payload p ON p.payload_id = w.payload_id
 WHERE w.account = ?1 AND p.deleted = 0 AND r.attempt_id IN (SELECT record_id FROM attempts)
 UNION
 SELECT w.kind, w.record_id FROM local_working w
 JOIN service_observation o ON o.payload_id = w.payload_id
 JOIN payload p ON p.payload_id = w.payload_id
 WHERE w.account = ?1 AND p.deleted = 0 AND o.external_id IN (SELECT external_id FROM associations)
 UNION
 SELECT w.kind, w.record_id FROM local_working w
 JOIN service_disposition d ON d.payload_id = w.payload_id
 JOIN payload p ON p.payload_id = w.payload_id
 WHERE w.account = ?1 AND p.deleted = 0 AND d.association_id IN (SELECT record_id FROM associations)
 UNION
 SELECT 'destination', d.destination_id FROM destination_descriptor d JOIN roots r
 ON d.payload_id = (SELECT w.payload_id FROM local_working w WHERE w.account = ?1 AND w.kind = r.kind AND w.record_id = r.record_id)
), needed(kind, record_id) AS (
 SELECT kind, record_id FROM roots
 UNION
 SELECT r.kind, r.record_id FROM needed n
 JOIN local_working w ON w.account = ?1 AND w.kind = n.kind AND w.record_id = n.record_id
 JOIN support_requirement r ON r.payload_id = w.payload_id
)
SELECT kind, record_id FROM needed ORDER BY kind, record_id
