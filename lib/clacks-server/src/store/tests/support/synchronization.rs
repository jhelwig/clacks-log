//! Real native admissions and proposal inputs shared by synchronization scenarios.

use crate::{
    auth::oidc::VerifiedIdentity,
    enrollment::{ApprovalPurpose, AuthorizedClient},
    oauth::{config, store::OAuthStore},
    secrets::keys::WrappingKeys,
    store::{AccountStore, metadata},
};
use clacks_sync::{
    client::{
        ClientRecordFields, LocalCounter, OriginalBasis, PendingRecord, Preparation,
        PreparedChange, VersionCondition,
    },
    record::{Observation, RecordKey, RecordState},
};
use huskarl_core::dpop::DPoP;
use huskarl_crypto_native::asymmetric::signer::{GenerateAlgorithm, PrivateKey};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use time::Duration;

/// One isolated account with two actually admitted clients using the production issuer.
pub(crate) struct NativeAccount {
    pub(crate) store: AccountStore,
    pub(crate) first: AuthorizedClient,
    pub(crate) second: AuthorizedClient,
    pub(crate) principal: crate::auth::principal::Principal,
}

impl NativeAccount {
    /// Create actual authority once for the scenario, using production migrations and grants.
    pub(crate) async fn create() -> anyhow::Result<Self> {
        let migrate = PgPoolOptions::new()
            .max_connections(1)
            .connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?)
            .await?;
        crate::maintenance::migrate::run(&migrate).await?;
        migrate.close().await;
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&std::env::var("CLACKS_TEST_RUNTIME_DATABASE_URL")?)
            .await?;
        let store = AccountStore::new(pool.clone());
        let login = store
            .provision_login(
                VerifiedIdentity {
                    issuer: "https://sync-provider.example".into(),
                    subject: ulid::Ulid::generate().to_string(),
                    display_name: None,
                },
                Duration::days(14),
            )
            .await?;
        let principal = store
            .authenticate(&login, Duration::days(14))
            .await?
            .unwrap();
        let directory = tempfile::tempdir()?;
        let file = directory.path().join("wrapping-key");
        std::fs::write(&file, [31_u8; 32])?;
        let keys = Arc::new(WrappingKeys::load(&crate::config::WrappingKeyConfig {
            active: "test".into(),
            files: [("test".into(), file)].into(),
        })?);
        let oauth = OAuthStore::new(store.clone(), keys);
        let issuer = config::issuer(oauth.clone(), &"https://clacks.example".parse()?);
        let dpop = DPoP::builder()
            .signer(PrivateKey::generate(GenerateAlgorithm::Es256, None)?)
            .build();
        let first = crate::store::tests::enrollment::approve(
            &issuer,
            &oauth,
            &principal,
            &dpop,
            "First desktop",
            ApprovalPurpose::New,
        )
        .await?;
        let second = crate::store::tests::enrollment::approve(
            &issuer,
            &oauth,
            &principal,
            &dpop,
            "Second desktop",
            ApprovalPurpose::New,
        )
        .await?;
        let first = oauth
            .authorized(&first.response.access_token)
            .await?
            .unwrap();
        let second = oauth
            .authorized(&second.response.access_token)
            .await?
            .unwrap();
        Ok(Self {
            store,
            first,
            second,
            principal,
        })
    }

    /// Prepare through the native production transition against actual accepted observations.
    pub(crate) async fn prepare(
        &self,
        client: &AuthorizedClient,
        key: RecordKey,
        state: RecordState,
    ) -> anyhow::Result<PreparedChange> {
        let mut transaction = self.store.pool.begin().await?;
        let account = metadata::account::load(&mut transaction, client.account).await?;
        let owner = key.account.to_string();
        let kind = crate::store::payload::kind(key.id);
        let id = key.id.to_string();
        let latest = match sqlx::query_file!("sql/synchronization/head_select.sql", owner, kind, id)
            .fetch_optional(&mut *transaction)
            .await?
        {
            Some(row) => Observation::Accepted(
                metadata::accepted::load_envelope(&mut transaction, key, row.envelope_id).await?,
            ),
            None => Observation::Absent {
                key,
                checkpoint: crate::store::synchronization::checkpoint(&account),
            },
        };
        let original = match &latest {
            Observation::Accepted(value) => OriginalBasis::Accepted(value.clone()),
            Observation::Absent { checkpoint, .. } => OriginalBasis::NeverCreated {
                key,
                checkpoint: Some(checkpoint.clone()),
            },
            Observation::Unavailable { .. } => unreachable!("actual server view"),
        };
        let record = ClientRecordFields {
            key,
            latest,
            retained: vec![],
            pending: Some(PendingRecord {
                key,
                original,
                working: state.clone(),
                prepared: None,
                witness: None,
            }),
        }
        .try_into()?;
        let mut dependencies = Vec::new();
        for reference in clacks_sync::acceptance::required_references(key, &state) {
            let kind = crate::store::payload::kind(reference.id);
            let id = reference.id.to_string();
            let row = sqlx::query_file!("sql/synchronization/head_select.sql", owner, kind, id)
                .fetch_one(&mut *transaction)
                .await?;
            dependencies.push(VersionCondition::from_observation(&Observation::Accepted(
                metadata::accepted::load_envelope(&mut transaction, reference, row.envelope_id)
                    .await?,
            ))?);
        }
        let interpretation =
            metadata::account::interpretation(&mut transaction, client.account).await?;
        let preparation = clacks_sync::client::preparation::prepare_change(
            &record,
            client.actor,
            &LocalCounter {
                actor: client.actor,
                high_water: 0,
            },
            &dependencies,
            &interpretation.view(),
        )?;
        let Preparation::Ready { change, .. } = preparation else {
            anyhow::bail!("fixture proposal was not ready");
        };
        transaction.commit().await?;
        Ok(change)
    }
}

/// Build a valid recorded contact without borrowing current operating defaults.
pub(crate) fn qso(
    id: clacks_model::ids::QsoId,
    tags: std::collections::BTreeSet<clacks_model::ids::TagId>,
) -> clacks_model::contact::Qso {
    use clacks_model::{
        contact::{BandFrequency, OwnOperatingContext, StationSnapshot, input::QsoFields},
        recognition::{
            RecognitionCatalog,
            mode::{Mode, ModeIdentity},
        },
        scalar::{Hertz, RecordedCallsign, UtcSecond},
    };
    QsoFields {
        id,
        start: UtcSecond::from_unix_seconds(1000),
        end: None,
        own_station: StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX").unwrap(),
            location: None,
            references: Default::default(),
        },
        contacted_station: StationSnapshot {
            callsign: RecordedCallsign::new("K1ABC").unwrap(),
            location: None,
            references: Default::default(),
        },
        own_context: OwnOperatingContext::default(),
        contacted_operator: None,
        primary: BandFrequency::new(
            Some(Hertz::new(14_074_000).unwrap()),
            None,
            RecognitionCatalog::adif_318().unwrap(),
        )
        .unwrap(),
        receive_override: None,
        mode: Mode::from_recorded_identity(ModeIdentity::Parent { label: "CW".into() }).unwrap(),
        sent_report: None,
        received_report: None,
        comment: None,
        tags,
    }
    .try_into()
    .unwrap()
}
