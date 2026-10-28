INSERT INTO contact_query_facts (
 qso_id, name,
 own_kind, own_locator, own_latitude, own_longitude, own_grid,
 contact_kind, contact_locator, contact_latitude, contact_longitude, contact_grid,
 lotw_uploaded, lotw_confirmed, qrz_uploaded, qrz_confirmed
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
