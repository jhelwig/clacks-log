INSERT INTO clacks.coupled_unit(account, operation_id) VALUES ($1,$2) ON CONFLICT DO NOTHING
