SELECT group_id, identifier FROM clacks.reported_reference WHERE report_id = $1 AND account = $2 ORDER BY group_id, identifier
