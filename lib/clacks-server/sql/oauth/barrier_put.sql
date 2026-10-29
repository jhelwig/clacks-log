INSERT INTO clacks.oauth_revocations (scope, target, recorded_at, until_at)
VALUES ($1,$2,$3,$4) ON CONFLICT (scope, target) DO UPDATE SET
recorded_at=GREATEST(clacks.oauth_revocations.recorded_at, EXCLUDED.recorded_at),
until_at=GREATEST(clacks.oauth_revocations.until_at, EXCLUDED.until_at)
