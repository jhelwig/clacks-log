//! Diagnostic reasons distinguish noncommitting settings outcomes from mutations.

use crate::settings_support;
use crate::support;
use crate::telemetry;

use clacks_client_store::{
    operating::{self, OperatingSaveOutcome, OperatingState},
    save,
    settings::{self, EquipmentDeleteOutcome, ProfileEquipmentReplacement},
    store::{ClientStore, StoreSettings},
};
use clacks_model::scalar::Milliwatts;
use clacks_sync::record::{RecordId, RecordKey};
use settings_support::{equipment, operating as state, profile};
use support::id;

#[test]
fn stale_apply_and_deletion_report_reasons_without_counting_mutations() -> anyhow::Result<()> {
    let captured = telemetry::capture_filtered(
        || -> anyhow::Result<()> {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    let directory = tempfile::tempdir()?;
                    let store = ClientStore::create(
                        &directory.path().join("account.sqlite"),
                        id(1),
                        StoreSettings::default(),
                    )
                    .await?;
                    let mut a = equipment(31, "PRIVATE RIG A");
                    let mut b = equipment(32, "PRIVATE RIG B");
                    settings::save_equipment(&store, None, &a).await?;
                    settings::save_equipment(&store, None, &b).await?;
                    let home = profile(30, Some(31));
                    settings::save_profile(&store, None, &home).await?;
                    let applied = state(&home, &a);
                    operating::save(&store, &OperatingState::default(), &applied).await?;
                    let mut other = applied.clone();
                    let mut values = other.current().cloned().unwrap();
                    values.transmit_power = Some(Milliwatts::new(25_000)?);
                    other.adjust(values);
                    operating::save(&store, &applied, &other).await?;
                    assert!(matches!(
                        operating::save(&store, &applied, &applied).await?,
                        OperatingSaveOutcome::Stale(_)
                    ));

                    let preview = settings::preview_equipment_deletion(&store, a.id()).await?;
                    let row = save::load(
                        &store,
                        RecordKey {
                            account: id(1),
                            id: RecordId::Equipment(a.id()),
                        },
                    )
                    .await?
                    .unwrap();
                    {
                        let mut fields = a.clone().into_fields();
                        fields.values.notes = Some("PRIVATE RIG NOTES".into());
                        a = fields.try_into().expect("valid saved fixture edit");
                    }
                    settings::save_equipment(&store, Some(&row), &a).await?;
                    let choices = [ProfileEquipmentReplacement {
                        profile: home.id(),
                        equipment: Some(b.id()),
                    }];
                    assert!(matches!(
                        settings::delete_equipment(&store, &preview, &choices).await?,
                        EquipmentDeleteOutcome::Stale(_)
                    ));
                    let preview = settings::preview_equipment_deletion(&store, a.id()).await?;
                    settings::save_profile(&store, None, &profile(33, Some(31))).await?;
                    assert!(matches!(
                        settings::delete_equipment(&store, &preview, &choices).await?,
                        EquipmentDeleteOutcome::Stale(_)
                    ));
                    let preview = settings::preview_equipment_deletion(&store, a.id()).await?;
                    let row = save::load(
                        &store,
                        RecordKey {
                            account: id(1),
                            id: RecordId::Equipment(b.id()),
                        },
                    )
                    .await?
                    .unwrap();
                    {
                        let mut fields = b.clone().into_fields();
                        fields.values.notes = Some("PRIVATE REPLACEMENT NOTES".into());
                        b = fields.try_into().expect("valid saved fixture edit");
                    }
                    settings::save_equipment(&store, Some(&row), &b).await?;
                    let choices = [
                        ProfileEquipmentReplacement {
                            profile: home.id(),
                            equipment: Some(b.id()),
                        },
                        ProfileEquipmentReplacement {
                            profile: id(33),
                            equipment: None,
                        },
                    ];
                    assert!(matches!(
                        settings::delete_equipment(&store, &preview, &choices).await?,
                        EquipmentDeleteOutcome::Stale(_)
                    ));
                    store.close().await?;
                    Ok(())
                })
        },
        |metadata| *metadata.level() <= tracing::Level::INFO,
    );
    captured
        .result
        .as_ref()
        .map_err(|e| anyhow::anyhow!("{e:#}"))?;
    assert_eq!(
        captured
            .event("operating.apply_deferred")
            .fields
            .get("reason")
            .map(String::as_str),
        Some("active_state_changed")
    );
    for reason in ["target_changed", "referrers_changed", "replacement_changed"] {
        let event = captured
            .observations
            .events
            .iter()
            .find(|e| {
                e.fields
                    .get("event")
                    .is_some_and(|v| v == "equipment.deletion_review_changed")
                    && e.fields.get("reason").is_some_and(|v| v == reason)
            })
            .unwrap_or_else(|| panic!("missing deletion reason {reason}"));
        assert!(event.fields.contains_key("account_id"));
        assert!(event.fields.contains_key("equipment_id"));
    }
    captured.counter("clacks_client_operating_applications_total", &[], 2);
    captured.counter(
        "clacks_client_equipment_mutations_total",
        &[("change", "create")],
        2,
    );
    captured.counter(
        "clacks_client_equipment_mutations_total",
        &[("change", "update")],
        2,
    );
    captured.counter(
        "clacks_client_profile_mutations_total",
        &[("change", "create")],
        2,
    );
    assert!(
        !captured
            .metrics
            .iter()
            .any(|m| m.labels.get("change").is_some_and(|v| v == "delete"))
    );
    let observations = format!("{:?}", captured.observations);
    for text in [
        "PRIVATE RIG A",
        "PRIVATE RIG B",
        "PRIVATE RIG NOTES",
        "PRIVATE REPLACEMENT NOTES",
        "W7HEX",
    ] {
        assert!(!observations.contains(text));
    }
    Ok(())
}
