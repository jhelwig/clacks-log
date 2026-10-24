CREATE INDEX qso_time ON qso_payload(account,start,record_id);
CREATE INDEX qso_tag_lookup ON qso_tag(tag_id,payload_id);
CREATE INDEX station_call_prefix ON station_snapshot(account,callsign COLLATE NOCASE);
CREATE INDEX accepted_identity ON accepted_record(account,kind,record_id);
CREATE INDEX support_stamp ON support_requirement(stamp_id);
CREATE INDEX stamp_baseline ON version_stamp(account,baseline_id);
