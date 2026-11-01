SELECT installation_order, timeline_id, revision FROM clacks.installation_journal
WHERE account = $1 ORDER BY installation_order
LIMIT least(greatest((SELECT count(*) FROM clacks.installation_journal WHERE account = $1) - $2, 0), $3)
