DELETE FROM clacks.oauth_revocations WHERE (scope,target) IN (SELECT scope,target FROM clacks.oauth_revocations WHERE until_at <= $1 LIMIT 500)
