//! Checked WS binary completion messages and their source-scoped correlation key.
mod adi;
mod binary;

use clacks_model::scalar::UtcSecond;
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
};

/// Source identity supplied by transport and the opaque case-sensitive instance Id.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SourceIdentity {
    /// Sender host; transport port is deliberately excluded.
    pub sender: IpAddr,
    /// WS protocol instance name.
    pub instance: String,
}

/// Exact radio discriminator, without invented frequencies for band-only messages.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PrimaryIdentity {
    /// Supplied whole hertz.
    Frequency(u64),
    /// Normalized supplied band when frequency is absent.
    Band(String),
}

/// Source completion key independent of optional details and local defaults.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CompletionIdentity {
    /// Trimmed uppercase call preserving all slash components.
    pub contacted_callsign: String,
    /// UTC start truncated to whole seconds.
    pub start: UtcSecond,
    /// Authoritative supplied frequency or normalized band.
    pub primary: PrimaryIdentity,
    /// Leaf mode label, using SUBMODE when supplied.
    pub mode: String,
}

/// Representation kind; pairing prefers a usable ADIF record as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompletionFormat {
    /// Structured type 5 fields.
    Structured,
    /// Wrapped length-framed type 12 ADI record.
    Adif,
}

impl CompletionFormat {
    /// Bounded diagnostic/metric dimension.
    pub fn label(self) -> &'static str {
        match self {
            Self::Structured => "structured",
            Self::Adif => "adif",
        }
    }
}

/// Protocol facts with missing text retained separately from invalid nonempty values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IncomingFacts {
    /// Named supported wire fields, never a saved contact or retained raw packet.
    pub values: BTreeMap<String, String>,
    /// Required represented UTC start.
    pub start: UtcSecond,
    /// Optional represented end, subject to native ordering at conversion.
    pub end: Option<UtcSecond>,
    /// Field names of supplied values that could not be interpreted.
    pub warnings: Vec<String>,
}

impl IncomingFacts {
    /// Borrow a nonblank value; empty live station fields are absent for fallback.
    pub fn text(&self, name: &str) -> Option<&str> {
        self.values
            .get(name)
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
    }
}

/// One identified completion representation ready for native projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DecodedCompletion {
    /// Transport/application source.
    pub source: SourceIdentity,
    /// Exact completion discriminator.
    pub identity: CompletionIdentity,
    /// Structured or ADIF representation.
    pub format: CompletionFormat,
    /// Supplied native-input facts.
    pub facts: IncomingFacts,
}

/// Decode one datagram, ignoring unrelated message types without heartbeat discovery.
/// Malformed/unsupported input returns a bounded error without a packet dump.
pub(crate) fn decode(
    datagram: &[u8],
    sender: SocketAddr,
) -> anyhow::Result<Option<DecodedCompletion>> {
    let mut input = datagram;
    anyhow::ensure!(
        binary::u32(&mut input)? == 0xadbc_cbda,
        "Invalid WS message magic."
    );
    let schema = binary::u32(&mut input)?;
    anyhow::ensure!(matches!(schema, 2 | 3), "Unsupported WS schema.");
    let kind = binary::u32(&mut input)?;
    if !matches!(kind, 5 | 12) {
        return Ok(None);
    }
    let instance = binary::text(&mut input)?;
    let (format, facts) = if kind == 5 {
        (
            CompletionFormat::Structured,
            binary::structured(&mut input)?,
        )
    } else {
        (
            CompletionFormat::Adif,
            adi::decode(binary::bytes(&mut input)?)?,
        )
    };
    let call = facts
        .text("CALL")
        .ok_or_else(|| anyhow::anyhow!("Missing contacted callsign."))?
        .trim()
        .to_ascii_uppercase();
    anyhow::ensure!(call.is_ascii(), "Non-ASCII contacted callsign.");
    let mode = facts
        .text("SUBMODE")
        .or_else(|| facts.text("MODE"))
        .ok_or_else(|| anyhow::anyhow!("Missing mode."))?
        .trim()
        .to_ascii_uppercase();
    let primary = if let Some(value) = facts.text("FREQ") {
        let value = crate::logging::radio::parse_mhz(value)?
            .ok_or_else(|| anyhow::anyhow!("Missing frequency."))?;
        PrimaryIdentity::Frequency(value.get())
    } else {
        PrimaryIdentity::Band(
            facts
                .text("BAND")
                .ok_or_else(|| anyhow::anyhow!("Missing frequency or band."))?
                .trim()
                .to_ascii_uppercase(),
        )
    };
    let identity = CompletionIdentity {
        contacted_callsign: call,
        start: facts.start,
        primary,
        mode,
    };
    Ok(Some(DecodedCompletion {
        source: SourceIdentity {
            sender: sender.ip(),
            instance,
        },
        identity,
        format,
        facts,
    }))
}
