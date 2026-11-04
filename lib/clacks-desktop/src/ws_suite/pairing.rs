//! Source-scoped 100 ms pairing and latest-successful-completion suppression.
use super::{
    contact::{self, CaptureContext, CaptureProjection},
    protocol::{CompletionFormat, CompletionIdentity, DecodedCompletion, SourceIdentity},
};
use clacks_model::ids::QsoId;
use std::{collections::BTreeMap, time::Duration};
use tokio::time::Instant;

/// Source and completion together name one transient candidate.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CandidateKey {
    /// Transport/application source.
    pub source: SourceIdentity,
    /// Exact contact discriminator.
    pub completion: CompletionIdentity,
}

/// Admission decision controls snapshot acquisition without allocating IDs for repeats.
pub(crate) enum Admission {
    /// First usable wire identity; acquire its local context once.
    New(CandidateKey),
    /// Representation joined an admitted candidate.
    Joined(CandidateKey),
    /// In-flight or latest-completed notification requires no work.
    Suppressed,
}

/// One representation and its optional checked native projection.
struct Representation {
    /// Original decoded facts needed until context arrives.
    notice: DecodedCompletion,
    /// Checked projection, retaining a bounded rejection rather than a raw error dump.
    projection: Option<Result<CaptureProjection, String>>,
}

/// Candidate ownership lasts through the terminal save outcome.
struct Candidate {
    /// Fresh native ID shared by both representations and the eventual save.
    id: QsoId,
    /// Original receipt time, independent of snapshot latency.
    received: Instant,
    /// Absolute deadline, never extended by a repeat.
    deadline: Instant,
    /// The original window has expired, even if its context read is still pending.
    window_closed: bool,
    /// One snapshot captured for both representations.
    context: Option<CaptureContext>,
    /// First structured representation.
    structured: Option<Representation>,
    /// First ADIF representation.
    adif: Option<Representation>,
    /// Selection was taken and is still saving.
    saving: bool,
}

/// Finalized selection retained as admitted until its save/rejection settles.
pub(crate) struct ReadyCapture {
    /// Candidate still recognized by the pairer.
    pub key: CandidateKey,
    /// Stable native identity.
    pub id: QsoId,
    /// Original monotonic receipt time for workflow duration.
    pub received: Instant,
    /// Selected usable format, absent when neither projection is usable.
    pub format: Option<CompletionFormat>,
    /// Native contact or actionable rejection.
    pub projection: Result<CaptureProjection, String>,
}

/// All transient completion state for one workspace receiver owner.
#[derive(Default)]
pub(crate) struct Pairing {
    /// Waiting and saving candidates, with no independent in-flight cache.
    candidates: BTreeMap<CandidateKey, Candidate>,
    /// Latest successful identity per source until the next success.
    completed: BTreeMap<SourceIdentity, CompletionIdentity>,
}

impl Pairing {
    /// Whether this packet may join existing work while admission is draining.
    pub fn contains(&self, notice: &DecodedCompletion) -> bool {
        self.candidates.contains_key(&CandidateKey {
            source: notice.source.clone(),
            completion: notice.identity.clone(),
        })
    }

    /// Recognize duplicates and keep the first representation of each type.
    pub fn admit(&mut self, notice: DecodedCompletion, received: Instant) -> Admission {
        if self.completed.get(&notice.source) == Some(&notice.identity) {
            tracing::debug!(event = "ws_suite.repeat_suppressed");
            return Admission::Suppressed;
        }
        let key = CandidateKey {
            source: notice.source.clone(),
            completion: notice.identity.clone(),
        };
        let new = !self.candidates.contains_key(&key);
        let candidate = self
            .candidates
            .entry(key.clone())
            .or_insert_with(|| Candidate {
                id: QsoId::new(),
                received,
                deadline: received + Duration::from_millis(100),
                window_closed: false,
                context: None,
                structured: None,
                adif: None,
                saving: false,
            });
        if candidate.saving || candidate.window_closed {
            return Admission::Suppressed;
        }
        let slot = match notice.format {
            CompletionFormat::Adif => &mut candidate.adif,
            CompletionFormat::Structured => &mut candidate.structured,
        };
        if let Some(first) = slot {
            if first.notice.facts != notice.facts {
                tracing::warn!(event="ws_suite.changed_repeat",source_ip=%key.source.sender,source_id=%key.source.instance);
            }
        } else {
            let projection = candidate.context.as_ref().map(|context| {
                contact::build(candidate.id, &notice, context).map_err(|_| {
                    "The completed contact has missing or invalid required values.".into()
                })
            });
            *slot = Some(Representation { notice, projection });
        }
        if new {
            Admission::New(key)
        } else {
            Admission::Joined(key)
        }
    }

    /// Attach one local snapshot and prepare all received representations against it.
    pub fn attach_context(
        &mut self,
        key: &CandidateKey,
        context: CaptureContext,
    ) -> anyhow::Result<()> {
        let candidate = self
            .candidates
            .get_mut(key)
            .ok_or_else(|| anyhow::anyhow!("Capture candidate no longer exists."))?;
        for representation in [&mut candidate.structured, &mut candidate.adif]
            .into_iter()
            .flatten()
        {
            representation.projection = Some(
                contact::build(candidate.id, &representation.notice, &context).map_err(|_| {
                    "The completed contact has missing or invalid required values.".into()
                }),
            );
        }
        candidate.context = Some(context);
        Ok(())
    }

    /// Earliest open window, independent of local snapshot readiness.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.candidates
            .values()
            .filter(|candidate| !candidate.saving && !candidate.window_closed)
            .map(|candidate| candidate.deadline)
            .min()
    }

    /// Select both usable types early, or the available usable representation at timeout.
    pub fn take_ready(&mut self, now: Instant) -> Vec<ReadyCapture> {
        let mut ready = vec![];
        for (key, candidate) in &mut self.candidates {
            if now >= candidate.deadline {
                candidate.window_closed = true;
            }
            if candidate.saving || candidate.context.is_none() {
                continue;
            }
            let usable = |representation: &Option<Representation>| {
                representation
                    .as_ref()
                    .and_then(|value| value.projection.as_ref())
                    .and_then(|value| value.as_ref().ok())
                    .cloned()
            };
            let adif = usable(&candidate.adif);
            let structured = usable(&candidate.structured);
            if now < candidate.deadline && !(adif.is_some() && structured.is_some()) {
                continue;
            }
            let (format, projection) = if let Some(value) = adif {
                (Some(CompletionFormat::Adif), Ok(value))
            } else if let Some(value) = structured {
                (Some(CompletionFormat::Structured), Ok(value))
            } else {
                (
                    None,
                    Err("The completed contact has missing or invalid required values.".into()),
                )
            };
            candidate.saving = true;
            ready.push(ReadyCapture {
                key: key.clone(),
                id: candidate.id,
                received: candidate.received,
                format,
                projection,
            });
        }
        ready
    }

    /// Release terminal work; only acknowledged success replaces the latest identity.
    pub fn settle(&mut self, key: &CandidateKey, saved: bool) {
        self.candidates.remove(key);
        if saved {
            self.completed
                .insert(key.source.clone(), key.completion.clone());
        }
    }

    /// Whether graceful drain has no admitted candidate or save remaining.
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}
