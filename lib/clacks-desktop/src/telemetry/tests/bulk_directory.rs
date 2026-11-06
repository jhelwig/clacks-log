//! Real bulk operation metrics and privacy under the process-owned telemetry composition.
use crate::directory::lookup::bulk::report::BulkCounts;
use crate::{
    directory::{
        lookup::tests::{bulk::finish, support},
        test_support,
    },
    workspace::WorkspacePaths,
};
use clacks_client_store::{save, store::ClientStore};

/// Exercise a cache-backed commit, rejected credentials and the unprocessed remainder.
pub(super) async fn run(paths: &WorkspacePaths) -> anyhow::Result<BulkCounts> {
    let provider = test_support::fixture(vec![
        support::login(),
        support::found(),
        test_support::response(401, ""),
    ])
    .await?;
    let store =
        ClientStore::create(&paths.database, Default::default(), Default::default()).await?;
    let service =
        support::Service::from_store(provider.endpoint.clone(), store, tempfile::tempdir()?)?;
    service.client.request(support::key("OK2CQR")).wait().await;
    let mut ids = std::collections::BTreeSet::new();
    for (n, call) in [(1_u128, "OK2CQR"), (2, "WB7PRIVATE"), (3, "WB7LATER")] {
        let id = ulid::Ulid::from(n).to_string().parse()?;
        ids.insert(id);
        save::contacts::save(&service.store, None, support::contact(id, call), None).await?;
    }
    let mut run = service
        .client
        .start_bulk(ids, tokio::sync::watch::channel(()).0)?;
    let report = finish(&mut run).await?;
    assert_eq!(
        report.counts,
        BulkCounts {
            new_updated: 1,
            failed: 1,
            unprocessed: 1,
            ..Default::default()
        }
    );
    let counts = report.counts.clone();
    service.close().await?;
    Ok(counts)
}

/// Provider data is deliberately excluded from all configured sinks.
pub(super) fn exclude_private(bytes: &[u8]) {
    for value in [
        "secret&+ sentinel",
        "fixture-key",
        "OK2CQR",
        "WB7PRIVATE",
        "WB7LATER",
        "<QRZDatabase>",
        "The Boss",
    ] {
        assert!(
            !bytes
                .windows(value.len())
                .any(|window| window == value.as_bytes()),
            "private directory value entered telemetry: {value}"
        );
    }
}

/// Compare reported business outcomes with the actual configured exported measurements.
pub(super) fn verify(
    requests: &[super::collector::Request],
    expected: &BulkCounts,
) -> anyhow::Result<()> {
    use opentelemetry_proto::tonic::{
        collector::metrics::v1::ExportMetricsServiceRequest,
        common::v1::any_value,
        metrics::v1::{metric::Data, number_data_point::Value},
    };
    use prost::Message;
    let mut outcomes = std::collections::BTreeMap::new();
    for request in requests {
        exclude_private(&request.body);
        if request.path != "/v1/metrics" {
            continue;
        }
        let export = ExportMetricsServiceRequest::decode(request.body.as_slice())?;
        for metric in export
            .resource_metrics
            .into_iter()
            .flat_map(|resource| resource.scope_metrics)
            .flat_map(|scope| scope.metrics)
        {
            if metric.name != "clacks_directory_bulk_contacts_total" {
                continue;
            }
            let Some(Data::Sum(sum)) = metric.data else {
                panic!("contact outcome counter");
            };
            for point in sum.data_points {
                assert_eq!(
                    point.attributes.len(),
                    1,
                    "no contact/account/payload metric dimensions"
                );
                let label = point.attributes.first().unwrap();
                assert_eq!(label.key, "outcome");
                let Some(any_value::Value::StringValue(outcome)) =
                    label.value.as_ref().and_then(|v| v.value.as_ref())
                else {
                    panic!("bounded outcome category");
                };
                assert!(
                    [
                        "new_updated",
                        "unchanged",
                        "not_found",
                        "failed",
                        "unprocessed"
                    ]
                    .contains(&outcome.as_str())
                );
                if let Some(Value::AsInt(value)) = point.value {
                    outcomes.insert(outcome.clone(), value);
                }
            }
        }
    }
    assert_eq!(
        outcomes.get("new_updated"),
        Some(&(expected.new_updated as i64))
    );
    assert_eq!(outcomes.get("failed"), Some(&(expected.failed as i64)));
    assert_eq!(
        outcomes.get("unprocessed"),
        Some(&(expected.unprocessed as i64))
    );
    assert_eq!(outcomes.values().sum::<i64>(), 3);
    Ok(())
}
