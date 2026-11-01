DELETE FROM clacks.installation_journal WHERE account = $1 AND installation_order = ANY($2)
