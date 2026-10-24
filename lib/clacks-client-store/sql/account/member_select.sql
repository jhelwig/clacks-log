SELECT actor_id, ordinal, created_timeline, created_revision, closed_timeline, closed_revision, checkpoint_id FROM client_membership WHERE account = ? ORDER BY ordinal
