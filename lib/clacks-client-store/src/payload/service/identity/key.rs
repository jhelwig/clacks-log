//! Canonical, lossless exact-provider key encoding for database equality.

use clacks_model::{
    scalar::{RecordedCallsign, UtcSecond},
    service::destination::{LotwRecordKey, QrzRecordId, RemoteRecordId},
};
use serde::Deserialize;

/// Accepted provider label in the QRZ array format.
#[derive(Deserialize)]
enum QrzLabel {
    #[serde(rename = "qrz")]
    Qrz,
}

/// Accepted provider label in the LoTW array format.
#[derive(Deserialize)]
enum LotwLabel {
    #[serde(rename = "lotw")]
    Lotw,
}

/// Exact positional arrays defined by the private external-key storage codec.
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredKey {
    Qrz(QrzLabel, QrzRecordId),
    Lotw(
        LotwLabel,
        RecordedCallsign,
        RecordedCallsign,
        i64,
        String,
        String,
        String,
    ),
}

/// Encode native labels exactly, without recognition or numeric ID conversion.
pub(super) fn encode(key: &RemoteRecordId) -> anyhow::Result<String> {
    Ok(match key {
        RemoteRecordId::Qrz(id) => serde_json::to_string(&("qrz", id.as_str()))?,
        RemoteRecordId::Lotw(key) => serde_json::to_string(&(
            "lotw",
            &key.own_call,
            &key.worked_call,
            key.start.unix_seconds(),
            &key.band,
            &key.receive_band,
            &key.mode,
        ))?,
    })
}

/// Restore the complete key; matching windows and current catalogs play no role.
pub(super) fn decode(text: &str) -> anyhow::Result<RemoteRecordId> {
    Ok(match serde_json::from_str::<StoredKey>(text)? {
        StoredKey::Qrz(_, id) => RemoteRecordId::Qrz(id),
        StoredKey::Lotw(_, own_call, worked_call, start, band, receive_band, mode) => {
            RemoteRecordId::Lotw(LotwRecordKey {
                own_call,
                worked_call,
                start: UtcSecond::from_unix_seconds(start),
                band,
                receive_band,
                mode,
            })
        }
    })
}
