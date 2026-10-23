//! Scoped observation of Clacks reporting decisions through actual operations.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;

/// Structured values observed on one span or event.
#[derive(Clone, Debug, Default)]
pub struct Observation {
    pub name: String,
    pub target: String,
    pub id: Option<u64>,
    pub parent: Option<u64>,
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
impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Collector {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut row = Observation {
            name: attrs.metadata().name().into(),
            target: attrs.metadata().target().into(),
            id: Some(id.into_u64()),
            parent: attrs.parent().map(Id::into_u64).or_else(|| {
                attrs
                    .is_contextual()
                    .then(|| ctx.lookup_current().map(|span| span.id().into_u64()))
                    .flatten()
            }),
            ..Default::default()
        };
        attrs.record(&mut row);
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
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut row = Observation {
            name: event.metadata().name().into(),
            target: event.metadata().target().into(),
            parent: ctx.event_span(event).map(|span| span.id().into_u64()),
            ..Default::default()
        };
        event.record(&mut row);
        row.fields
            .insert("level".into(), event.metadata().level().to_string());
        self.0.lock().unwrap().events.push(row);
    }
}

pub fn capture<T>(action: impl FnOnce() -> T, spans: bool) -> (T, Observations) {
    let _cache_guard = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let collector = Collector::default();
    let subscriber = tracing_subscriber::registry().with(collector.clone().with_filter(
        tracing_subscriber::filter::filter_fn(move |m| spans || m.is_event()),
    ));
    let result = tracing::subscriber::with_default(subscriber, action);
    let observations = collector.0.lock().unwrap().clone();
    (result, observations)
}
