DELETE FROM clacks.desktop_code_approvals WHERE code_digest IN (
    SELECT a.code_digest FROM clacks.desktop_code_approvals a
    WHERE a.account_id = $1 AND a.expires_at <= $2
      AND NOT EXISTS (
          SELECT 1 FROM clacks.desktop_authorization_codes c
          WHERE c.code_digest = a.code_digest
      )
    LIMIT 500
)
