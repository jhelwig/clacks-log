CREATE TABLE local_review (
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    original_id INTEGER NOT NULL,
    local_payload INTEGER NOT NULL,
    shared_observation INTEGER NOT NULL,
    reason TEXT NOT NULL CHECK (reason IN ('different','original','ordering','reference','actor')),
    unknown_reason TEXT CHECK (unknown_reason IN ('baseline','lineage','original','collected')),
    reference_kind TEXT,
    reference_id TEXT,
    actor_id TEXT CHECK (actor_id IS NULL OR length(actor_id) = 26),
    PRIMARY KEY (account,kind,record_id),
    FOREIGN KEY (account,kind,record_id) REFERENCES client_record(account,kind,record_id),
    FOREIGN KEY (original_id,account,kind,record_id) REFERENCES original_basis(original_id,account,kind,record_id),
    FOREIGN KEY (local_payload,account,kind,record_id) REFERENCES payload(payload_id,account,kind,record_id),
    FOREIGN KEY (shared_observation,account,kind,record_id) REFERENCES record_observation(observation_id,account,kind,record_id),
    FOREIGN KEY (account,reference_kind,reference_id) REFERENCES record_identity(account,kind,record_id),
    CHECK ((reason IN ('different','original') AND unknown_reason IS NULL AND reference_kind IS NULL AND reference_id IS NULL AND actor_id IS NULL)
        OR (reason = 'ordering' AND unknown_reason IS NOT NULL AND reference_kind IS NULL AND reference_id IS NULL AND actor_id IS NULL)
        OR (reason = 'reference' AND unknown_reason IS NULL AND reference_kind IS NOT NULL AND reference_id IS NOT NULL AND actor_id IS NULL)
        OR (reason = 'actor' AND unknown_reason IS NULL AND reference_kind IS NULL AND reference_id IS NULL AND actor_id IS NOT NULL))
) STRICT;
CREATE TABLE review_retained (
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    envelope_id INTEGER NOT NULL,
    PRIMARY KEY (account,kind,record_id,ordinal),
    FOREIGN KEY (account,kind,record_id) REFERENCES local_review(account,kind,record_id) ON DELETE CASCADE,
    FOREIGN KEY (envelope_id,account,kind,record_id) REFERENCES accepted_record(envelope_id,account,kind,record_id)
) STRICT;
