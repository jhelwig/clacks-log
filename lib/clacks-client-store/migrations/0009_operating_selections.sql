CREATE TABLE operating_state_new (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    account TEXT NOT NULL UNIQUE REFERENCES store_account(account),
    current_values INTEGER,
    loaded_profile INTEGER,
    profile_kind TEXT NOT NULL DEFAULT 'profile' CHECK (profile_kind = 'profile'),
    profile_record TEXT,
    loaded_equipment INTEGER,
    equipment_kind TEXT NOT NULL DEFAULT 'equipment' CHECK (equipment_kind = 'equipment'),
    equipment_record TEXT,
    FOREIGN KEY (current_values, account) REFERENCES operating_value(operating_id, account),
    FOREIGN KEY (loaded_profile, account, profile_kind, profile_record) REFERENCES payload(payload_id, account, kind, record_id),
    FOREIGN KEY (loaded_equipment, account, equipment_kind, equipment_record) REFERENCES payload(payload_id, account, kind, record_id)
) STRICT;

-- Snapshot identities differ even for equal values. Compare the complete station,
-- including location presence and unordered references, before retaining a source.
WITH comparable AS (
    SELECT s.singleton, s.account, s.current_values, s.loaded_profile, s.profile_record,
        cs.callsign IS bs.callsign
        AND (cl.station_id IS NULL) = (bl.station_id IS NULL)
        AND NOT EXISTS (
            SELECT cl.position_kind, cl.locator, cl.latitude, cl.longitude, cl.grid_length,
                   cl.dxcc, cl.primary_subdivision, cl.secondary_subdivision, cl.cq_zone, cl.itu_zone
            EXCEPT
            SELECT bl.position_kind, bl.locator, bl.latitude, bl.longitude, bl.grid_length,
                   bl.dxcc, bl.primary_subdivision, bl.secondary_subdivision, bl.cq_zone, bl.itu_zone
        )
        AND NOT EXISTS (
            SELECT group_id, identifier FROM station_reference WHERE account = s.account AND station_id = cv.station_id
            EXCEPT
            SELECT group_id, identifier FROM station_reference WHERE account = s.account AND station_id = bv.station_id
        )
        AND NOT EXISTS (
            SELECT group_id, identifier FROM station_reference WHERE account = s.account AND station_id = bv.station_id
            EXCEPT
            SELECT group_id, identifier FROM station_reference WHERE account = s.account AND station_id = cv.station_id
        )
        AND NOT EXISTS (
            SELECT 1 FROM local_working w JOIN payload p ON p.payload_id = w.payload_id
            WHERE w.account = s.account AND w.kind = 'profile' AND w.record_id = s.profile_record AND p.deleted = 1
        ) AS retain
    FROM operating_state s
    LEFT JOIN operating_value cv ON cv.operating_id = s.current_values AND cv.account = s.account
    LEFT JOIN operating_value bv ON bv.operating_id = s.loaded_values AND bv.account = s.account
    LEFT JOIN station_snapshot cs ON cs.station_id = cv.station_id AND cs.account = s.account
    LEFT JOIN station_snapshot bs ON bs.station_id = bv.station_id AND bs.account = s.account
    LEFT JOIN recorded_location cl ON cl.station_id = cv.station_id AND cl.account = s.account
    LEFT JOIN recorded_location bl ON bl.station_id = bv.station_id AND bl.account = s.account
)
INSERT INTO operating_state_new (singleton, account, current_values, loaded_profile, profile_record)
SELECT singleton, account, current_values,
       CASE WHEN current_values IS NOT NULL AND retain THEN loaded_profile END,
       CASE WHEN current_values IS NOT NULL AND retain THEN profile_record END
FROM comparable;

DROP TABLE operating_state;
ALTER TABLE operating_state_new RENAME TO operating_state;
