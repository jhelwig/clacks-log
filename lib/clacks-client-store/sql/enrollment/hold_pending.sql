UPDATE client_record SET authorization_hold=COALESCE(authorization_hold, ?) WHERE account=? AND pending_id IS NOT NULL
