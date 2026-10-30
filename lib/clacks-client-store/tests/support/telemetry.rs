//! Scoped observation of Clacks reporting decisions through actual operations.

use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;

/// Structured values observed on one span or event.
#[derive(Clone, Debug, Default)]
pub struct Observation {
    pub id: Option<u64>,
    pub fields: BTreeMap<String, String>,
}
impl Visit for Observation {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().into(), format!("{value:?}"));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields.insert(field.name().into(), value.into());
    }
}

/// Span and structured event observations from a single scoped operation.
#[derive(Clone, Debug, Default)]
pub struct Observations {
    pub spans: Vec<Observation>,
    pub events: Vec<Observation>,
}
#[derive(Clone, Default)]
struct Collector(Arc<Mutex<Observations>>);
impl<S: Subscriber> Layer<S> for Collector {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, _ctx: Context<'_, S>) {
        let mut row = Observation {
            id: Some(id.into_u64()),
            ..Default::default()
        };
        attrs.record(&mut row);
        row.fields
            .insert("level".into(), attrs.metadata().level().to_string());
        self.0.lock().unwrap().spans.push(row);
    }
    fn on_record(&self, id: &Id, values: &Record<'_>, _: Context<'_, S>) {
        if let Some(row) = self
            .0
            .lock()
            .unwrap()
            .spans
            .iter_mut()
            .find(|row| row.id == Some(id.into_u64()))
        {
            values.record(row);
        }
    }
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut row = Observation::default();
        event.record(&mut row);
        row.fields
            .insert("level".into(), event.metadata().level().to_string());
        self.0.lock().unwrap().events.push(row);
    }
}

/// Named metric with its actual dimensions and measurements.
#[derive(Debug)]
pub struct Metric {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub value: DebugValue,
}
/// Returned domain value alongside structured observations.
#[derive(Debug)]
pub struct Capture<T> {
    pub result: T,
    pub observations: Observations,
    pub metrics: Vec<Metric>,
}
impl<T> Capture<T> {
    /// Find an operation's local diagnostic event by its explicit event name.
    pub fn event(&self, name: &str) -> &Observation {
        self.observations
            .events
            .iter()
            .find(|e| e.fields.get("event").is_some_and(|v| v == name))
            .expect("Clacks diagnostic event missing")
    }

    /// Check an operation-owned counter and its complete label set.
    pub fn counter(&self, name: &str, labels: &[(&str, &str)], expected: u64) {
        let labels: BTreeMap<_, _> = labels
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        let metric = self
            .metrics
            .iter()
            .find(|m| m.name == name && m.labels == labels)
            .expect("Clacks domain counter missing");
        assert!(matches!(metric.value, DebugValue::Counter(count) if count == expected));
    }
}
/// Run synchronous Clacks work without changing process-global telemetry state.
pub fn capture<T>(action: impl FnOnce() -> T) -> Capture<T> {
    capture_filtered(action, |_| true)
}
/// Filter telemetry for a concrete Clacks context or control-flow regression.
pub fn capture_filtered<T>(
    action: impl FnOnce() -> T,
    enabled: fn(&tracing::Metadata<'_>) -> bool,
) -> Capture<T> {
    // Keep two dispatchers alive: tracing-core's single-dispatch cache consults
    // the registering thread, which may be an unconfigured parallel test.
    let _cache_guard = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let collector = Collector::default();
    let subscriber = tracing_subscriber::registry().with(
        collector
            .clone()
            .with_filter(tracing_subscriber::filter::filter_fn(enabled)),
    );
    let recorder = DebuggingRecorder::new();
    let snapshot = recorder.snapshotter();
    let result = tracing::subscriber::with_default(subscriber, || {
        metrics::with_local_recorder(&recorder, action)
    });
    let observations = collector.0.lock().unwrap().clone();
    let metrics = snapshot
        .snapshot()
        .into_vec()
        .into_iter()
        .map(|(key, _, _, value)| Metric {
            name: key.key().name().into(),
            labels: key
                .key()
                .labels()
                .map(|l| (l.key().into(), l.value().into()))
                .collect(),
            value,
        })
        .collect();
    Capture {
        result,
        observations,
        metrics,
    }
}
