CREATE TABLE station_snapshot (
    station_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    callsign TEXT NOT NULL,
    UNIQUE (station_id, account)
) STRICT;

CREATE TABLE recorded_location (
    station_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    position_kind TEXT,
    locator TEXT,
    latitude INTEGER,
    longitude INTEGER,
    grid_length INTEGER,
    dxcc TEXT,
    primary_subdivision TEXT,
    secondary_subdivision TEXT,
    cq_zone INTEGER,
    itu_zone INTEGER,
    FOREIGN KEY (station_id, account) REFERENCES station_snapshot(station_id, account) ON DELETE CASCADE
) STRICT;

CREATE TABLE station_reference (
    reference_id INTEGER PRIMARY KEY,
    station_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    group_kind TEXT NOT NULL DEFAULT 'group' CHECK (group_kind = 'group'),
    group_id TEXT NOT NULL,
    identifier TEXT,
    FOREIGN KEY (station_id, account) REFERENCES station_snapshot(station_id, account) ON DELETE CASCADE,
    FOREIGN KEY (account, group_kind, group_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
CREATE UNIQUE INDEX station_reference_value ON station_reference(station_id, group_id, identifier) WHERE identifier IS NOT NULL;
CREATE UNIQUE INDEX station_reference_group ON station_reference(station_id, group_id) WHERE identifier IS NULL;

CREATE TABLE equipment_snapshot (
    equipment_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    equipment_description TEXT,
    antenna_description TEXT,
    notes TEXT,
    UNIQUE (equipment_id, account)
) STRICT;

CREATE TABLE qso_payload (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'qso' CHECK (kind = 'qso'),
    record_id TEXT NOT NULL,
    start INTEGER NOT NULL,
    end INTEGER,
    own_station INTEGER NOT NULL,
    contacted_station INTEGER NOT NULL,
    own_operator_call TEXT,
    own_operator_name TEXT,
    contacted_operator_call TEXT,
    contacted_operator_name TEXT,
    power INTEGER,
    equipment_id INTEGER,
    frequency INTEGER,
    band TEXT,
    receive_frequency INTEGER,
    receive_band TEXT,
    mode_kind TEXT NOT NULL,
    mode_label TEXT NOT NULL,
    mode_parent TEXT,
    sent_report TEXT,
    received_report TEXT,
    comment TEXT,
    UNIQUE (payload_id, account),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (own_station, account) REFERENCES station_snapshot(station_id, account),
    FOREIGN KEY (contacted_station, account) REFERENCES station_snapshot(station_id, account),
    FOREIGN KEY (equipment_id, account) REFERENCES equipment_snapshot(equipment_id, account)
) STRICT;

CREATE TABLE qso_tag (
    payload_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    tag_kind TEXT NOT NULL DEFAULT 'tag' CHECK (tag_kind = 'tag'),
    tag_id TEXT NOT NULL,
    PRIMARY KEY (payload_id, tag_id),
    FOREIGN KEY (payload_id, account) REFERENCES qso_payload(payload_id, account) ON DELETE CASCADE,
    FOREIGN KEY (account, tag_kind, tag_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
