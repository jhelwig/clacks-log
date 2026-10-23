#![allow(dead_code)]

pub mod telemetry;

use std::path::Path;

pub fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .expect("committed fixture")
}

pub fn now() -> time::OffsetDateTime {
    time::Date::from_calendar_date(2026, time::Month::October, 22)
        .unwrap()
        .with_hms(12, 34, 56)
        .unwrap()
        .assume_utc()
}

pub fn submission() -> clacks_model::service::lotw::LotwSubmission {
    use clacks_model::{
        ids::DestinationId,
        scalar::{RecordedCallsign, UtcSecond},
        service::{
            destination::LotwRecordKey,
            lotw::{LotwMappingProvenance, LotwSubmission},
        },
    };
    let identity = clacks_lotw::certificate::inspect_certificate(&fixture("1024/station.der"))
        .unwrap()
        .identity;
    LotwSubmission {
        mapping_provenance: LotwMappingProvenance {
            release: "11.34".into(),
            upstream_commit: "6ce824363e27da455c6cf6fd440b9c401945bfcb".into(),
        },
        destination: DestinationId::new(),
        signing_identity: identity.clone(),
        contact_fields: [
            ("CALL", "K1ABC"),
            ("BAND", "20M"),
            ("MODE", "FT8"),
            ("FREQ", "14.074000"),
            ("QSO_DATE", "20261022"),
            ("QSO_TIME", "123456"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect(),
        station_fields: [
            ("CQZ", "5"),
            ("GRIDSQUARE", "FN31"),
            ("ITUZ", "8"),
            ("US_STATE", "CT"),
            ("US_COUNTY", "Hartford"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect(),
        key: LotwRecordKey {
            own_call: identity.callsign,
            worked_call: RecordedCallsign::new("K1ABC").unwrap(),
            start: UtcSecond::from_unix_seconds(now().unix_timestamp()),
            band: "20M".into(),
            receive_band: "20M".into(),
            mode: "FT8".into(),
        },
    }
}

pub struct Field {
    pub name: String,
    pub kind: Option<String>,
    pub bytes: Vec<u8>,
}

pub fn fields(bytes: &[u8]) -> Vec<Field> {
    let mut remaining = bytes;
    let mut fields = Vec::new();
    while !remaining.is_empty() {
        remaining = remaining.trim_ascii_start();
        if remaining.is_empty() {
            break;
        }
        assert_eq!(remaining.first(), Some(&b'<'));
        let close = remaining.iter().position(|b| *b == b'>').unwrap();
        let header = std::str::from_utf8(&remaining[1..close]).unwrap();
        let mut parts = header.split(':');
        let name = parts.next().unwrap().to_ascii_uppercase();
        let len = parts
            .next()
            .map(|v| v.parse::<usize>().unwrap())
            .unwrap_or(0);
        let kind = parts.next().map(str::to_owned);
        assert!(parts.next().is_none());
        remaining = &remaining[close + 1..];
        let (value, rest) = remaining.split_at(len);
        fields.push(Field {
            name,
            kind,
            bytes: value.into(),
        });
        remaining = rest;
    }
    fields
}

pub struct LimitedWriter {
    pub bytes: Vec<u8>,
    pub chunk: usize,
    pub fail_at: Option<usize>,
    pub interrupted: bool,
}

impl std::io::Write for LimitedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.interrupted {
            self.interrupted = false;
            return Err(std::io::ErrorKind::Interrupted.into());
        }
        if self.fail_at.is_some_and(|n| self.bytes.len() >= n) {
            return Err(std::io::ErrorKind::BrokenPipe.into());
        }
        let count = buf.len().min(self.chunk).min(
            self.fail_at
                .map(|n| n - self.bytes.len())
                .unwrap_or(usize::MAX),
        );
        self.bytes.extend(buf.iter().copied().take(count));
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        panic!("serializer must not flush caller-owned compression");
    }
}
