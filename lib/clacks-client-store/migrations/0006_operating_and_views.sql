CREATE TABLE operating_value (
    operating_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    station_id INTEGER NOT NULL,
    operator_call TEXT,
    operator_name TEXT,
    power INTEGER,
    equipment_id INTEGER,
    UNIQUE (operating_id, account),
    FOREIGN KEY (station_id, account) REFERENCES station_snapshot(station_id, account),
    FOREIGN KEY (equipment_id, account) REFERENCES equipment_snapshot(equipment_id, account)
) STRICT;
CREATE TABLE operating_state (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    account TEXT NOT NULL UNIQUE REFERENCES store_account(account),
    current_values INTEGER,
    loaded_profile INTEGER,
    profile_kind TEXT NOT NULL DEFAULT 'profile' CHECK (profile_kind = 'profile'),
    profile_record TEXT,
    loaded_values INTEGER,
    FOREIGN KEY (current_values, account) REFERENCES operating_value(operating_id, account),
    FOREIGN KEY (loaded_values, account) REFERENCES operating_value(operating_id, account),
    FOREIGN KEY (loaded_profile, account, profile_kind, profile_record) REFERENCES payload(payload_id, account, kind, record_id)
) STRICT;

CREATE VIEW local_working AS
SELECT c.account, c.kind, c.record_id, coalesce(p.working_payload, a.payload_id) AS payload_id,
       CASE WHEN p.pending_id IS NULL THEN 0 ELSE 1 END AS pending
FROM client_record c
JOIN record_observation o ON o.observation_id = c.latest_observation
LEFT JOIN accepted_record a ON a.envelope_id = o.envelope_id
LEFT JOIN pending_record p ON p.pending_id = c.pending_id
WHERE p.working_payload IS NOT NULL OR a.payload_id IS NOT NULL;

CREATE VIEW local_qso AS
SELECT q.account, q.record_id, q.payload_id, w.pending, q.start, q.end,
       q.frequency, q.band, q.mode_kind, q.mode_label, q.mode_parent,
       own.callsign AS own_call, contacted.callsign AS worked_call
FROM qso_payload q
CROSS JOIN local_working w ON w.account = q.account AND w.kind = 'qso'
    AND w.record_id = q.record_id AND w.payload_id = q.payload_id
JOIN payload p ON p.payload_id = q.payload_id
JOIN station_snapshot own ON own.station_id = q.own_station
JOIN station_snapshot contacted ON contacted.station_id = q.contacted_station
WHERE w.kind = 'qso' AND p.deleted = 0;

CREATE UNIQUE INDEX accepted_envelope_payload ON accepted_record(envelope_id, account, kind, record_id, payload_id);
CREATE UNIQUE INDEX association_payload_external ON service_association(payload_id, account, external_id);
CREATE TABLE current_external_binding (
    account TEXT NOT NULL,
    external_id INTEGER NOT NULL,
    association_kind TEXT NOT NULL DEFAULT 'association' CHECK (association_kind = 'association'),
    association_id TEXT NOT NULL,
    envelope_id INTEGER NOT NULL,
    payload_id INTEGER NOT NULL,
    PRIMARY KEY (account, external_id),
    UNIQUE (account, association_id),
    FOREIGN KEY (external_id, account) REFERENCES external_key(external_id, account),
    FOREIGN KEY (envelope_id, account, association_kind, association_id, payload_id) REFERENCES accepted_record(envelope_id, account, kind, record_id, payload_id),
    FOREIGN KEY (payload_id, account, external_id) REFERENCES service_association(payload_id, account, external_id)
) STRICT;
