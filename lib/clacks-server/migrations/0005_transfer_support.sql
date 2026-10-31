CREATE TABLE clacks.current_external_binding (
    account TEXT NOT NULL,
    external_id BIGINT NOT NULL,
    association_kind TEXT NOT NULL DEFAULT 'association',
    association_id TEXT NOT NULL,
    envelope_id BIGINT NOT NULL,
    PRIMARY KEY (account, external_id),
    UNIQUE (account, association_id),
    FOREIGN KEY (external_id, account) REFERENCES clacks.external_key(external_id, account),
    FOREIGN KEY (envelope_id, account, association_kind, association_id) REFERENCES clacks.accepted_record(envelope_id, account, kind, record_id)
);
CREATE TABLE clacks.coupled_unit (
    account TEXT NOT NULL REFERENCES clacks.accounts,
    operation_id TEXT NOT NULL,
    PRIMARY KEY (account, operation_id)
);
CREATE TABLE clacks.coupled_member (
    account TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    ordinal BIGINT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    stamp_id BIGINT NOT NULL,
    deleted BOOLEAN NOT NULL,
    PRIMARY KEY (account, operation_id, ordinal),
    FOREIGN KEY (account, operation_id) REFERENCES clacks.coupled_unit ON DELETE CASCADE,
    FOREIGN KEY (stamp_id, account, kind, record_id) REFERENCES clacks.version_stamp(stamp_id, account, kind, record_id)
);
CREATE INDEX coupled_member_record ON clacks.coupled_member(account, kind, record_id);
ALTER TABLE clacks.sync_baselines ADD FOREIGN KEY (coverage_id, account_id) REFERENCES clacks.coverage(coverage_id, account);
ALTER TABLE clacks.sync_baselines ADD FOREIGN KEY (lineage_id, account_id) REFERENCES clacks.lineage(lineage_id, account);
ALTER TABLE clacks.sync_accounts ADD FOREIGN KEY (installed_coverage_id, account_id) REFERENCES clacks.coverage(coverage_id, account);
ALTER TABLE clacks.sync_accounts ADD FOREIGN KEY (current_lineage_id, account_id) REFERENCES clacks.lineage(lineage_id, account);
ALTER TABLE clacks.sync_accounts ADD FOREIGN KEY (retained_journal_coverage_id, account_id) REFERENCES clacks.coverage(coverage_id, account);
ALTER TABLE clacks.desktop_clients ADD FOREIGN KEY (acknowledged_checkpoint_id, account_id) REFERENCES clacks.checkpoint(checkpoint_id, account);
