CREATE TEMP TABLE contact_query_facts (
 qso_id TEXT PRIMARY KEY,
 name TEXT,
 own_kind TEXT, own_locator TEXT, own_latitude INTEGER, own_longitude INTEGER,
 own_grid TEXT,
 contact_kind TEXT, contact_locator TEXT, contact_latitude INTEGER, contact_longitude INTEGER,
 contact_grid TEXT,
 lotw_uploaded INTEGER NOT NULL, lotw_confirmed INTEGER NOT NULL,
 qrz_uploaded INTEGER NOT NULL, qrz_confirmed INTEGER NOT NULL
) STRICT
