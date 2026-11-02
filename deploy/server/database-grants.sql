-- Run as the schema owner after release migrations.
GRANT USAGE ON SCHEMA clacks TO clacks_runtime, clacks_maintenance;
-- UPDATE on accounts is required by PostgreSQL's account-row locking.
GRANT SELECT, INSERT, UPDATE ON clacks.accounts TO clacks_runtime;
GRANT SELECT, INSERT, DELETE ON clacks.login_identities TO clacks_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON clacks.login_sessions TO clacks_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON clacks.browser_sessions TO clacks_runtime;
GRANT SELECT ON clacks.server_administrators TO clacks_runtime;
GRANT SELECT, INSERT, UPDATE ON clacks.account_encryption_keys,
    clacks.sync_accounts, clacks.sync_baselines,
    clacks.desktop_clients TO clacks_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON clacks.desktop_code_approvals,
    clacks.desktop_authorization_codes, clacks.desktop_code_scopes,
    clacks.desktop_code_resources, clacks.desktop_grants,
    clacks.desktop_access_tokens, clacks.desktop_access_scopes,
    clacks.desktop_access_resources, clacks.desktop_refresh_tokens,
    clacks.desktop_refresh_scopes, clacks.desktop_refresh_resources,
    clacks.oauth_revocations, clacks.oauth_replay_claims,
    clacks.resource_replay_claims TO clacks_runtime;
GRANT SELECT, UPDATE ON clacks.accounts TO clacks_maintenance;
GRANT SELECT, INSERT ON clacks.login_identities TO clacks_maintenance;
GRANT SELECT, INSERT, DELETE ON clacks.server_administrators TO clacks_maintenance;
GRANT SELECT, INSERT, UPDATE ON clacks.account_encryption_keys TO clacks_maintenance;
GRANT SELECT ON clacks.desktop_authorization_codes,
    clacks.desktop_code_approvals TO clacks_maintenance;
GRANT UPDATE (receipt)
    ON clacks.desktop_authorization_codes TO clacks_maintenance;
-- Immutable native payloads and causal descriptions are read or appended.
GRANT SELECT, INSERT ON clacks.record_identity TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.payload TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.station_snapshot TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.recorded_location TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.station_reference TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.equipment_snapshot TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.qso_payload TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.qso_tag TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.tag_payload TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.profile_payload TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.equipment_payload TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.output_profile_payload TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.group_payload TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.export_plan TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.record_vector TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.version_stamp TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.destination_descriptor TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.external_key TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.service_support TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.support_requirement TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.service_destination TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.service_association TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.service_suppression TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.service_disposition TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.service_query TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.service_download TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.stream_progress TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.service_acquisition TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.acquisition_input TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.submission_projection TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.service_attempt TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.service_result TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.result_member TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.contact_report TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.station_report TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.reported_reference TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.service_observation TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.imported_location TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.confirmation_basis TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.service_assertion TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.coverage TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.lineage TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.timeline_boundary TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.checkpoint TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.accepted_record TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.installation_journal TO clacks_runtime;
GRANT SELECT, INSERT, UPDATE ON clacks.accepted_head TO clacks_runtime;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA clacks TO clacks_runtime;
GRANT SELECT, INSERT, DELETE ON clacks.current_external_binding TO clacks_runtime;
GRANT SELECT, INSERT ON clacks.coupled_unit, clacks.coupled_member TO clacks_runtime;
GRANT SELECT, INSERT, DELETE ON clacks.staged_accepted, clacks.retained_accepted TO clacks_runtime;
-- Baseline installation only repoints the equivalent accepted representation.
GRANT UPDATE (envelope_id) ON clacks.current_external_binding TO clacks_runtime;
-- Account retention deletes only unreachable native representations and their
-- exclusively owned child rows; identities and authorization remain intact.
GRANT DELETE ON clacks.accepted_record, clacks.payload,
    clacks.station_snapshot, clacks.equipment_snapshot,
    clacks.version_stamp, clacks.record_vector, clacks.checkpoint,
    clacks.sync_baselines, clacks.lineage, clacks.coverage,
    clacks.installation_journal TO clacks_runtime;
