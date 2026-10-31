CREATE TABLE clacks.staged_accepted (
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    envelope_id BIGINT NOT NULL,
    PRIMARY KEY(account, envelope_id),
    FOREIGN KEY(envelope_id, account, kind, record_id) REFERENCES clacks.accepted_record(envelope_id, account, kind, record_id)
);
CREATE TABLE clacks.retained_accepted (
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    envelope_id BIGINT NOT NULL,
    PRIMARY KEY(account, envelope_id),
    FOREIGN KEY(envelope_id, account, kind, record_id) REFERENCES clacks.accepted_record(envelope_id, account, kind, record_id)
);
CREATE INDEX retained_accepted_identity ON clacks.retained_accepted(account, kind, record_id);
