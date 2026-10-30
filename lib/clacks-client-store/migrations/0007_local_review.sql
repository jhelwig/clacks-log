CREATE TABLE local_review (
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    original_id INTEGER NOT NULL,
    local_payload INTEGER NOT NULL,
    shared_observation INTEGER NOT NULL,
    reason TEXT NOT NULL,
    unknown_reason TEXT,
    reference_kind TEXT,
    reference_id TEXT,
    actor_id TEXT,
    PRIMARY KEY (account,kind,record_id),
    FOREIGN KEY (account,kind,record_id) REFERENCES client_record(account,kind,record_id),
    FOREIGN KEY (original_id,account,kind,record_id) REFERENCES original_basis(original_id,account,kind,record_id),
    FOREIGN KEY (local_payload,account,kind,record_id) REFERENCES payload(payload_id,account,kind,record_id),
    FOREIGN KEY (shared_observation,account,kind,record_id) REFERENCES record_observation(observation_id,account,kind,record_id),
    FOREIGN KEY (account,reference_kind,reference_id) REFERENCES record_identity(account,kind,record_id)
) STRICT;
CREATE TABLE review_retained (
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    envelope_id INTEGER NOT NULL,
    PRIMARY KEY (account,kind,record_id,ordinal),
    FOREIGN KEY (account,kind,record_id) REFERENCES local_review(account,kind,record_id) ON DELETE CASCADE,
    FOREIGN KEY (envelope_id,account,kind,record_id) REFERENCES accepted_record(envelope_id,account,kind,record_id)
) STRICT;
