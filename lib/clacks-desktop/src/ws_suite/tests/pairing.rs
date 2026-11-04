use super::super::{
    pairing::{Admission, Pairing},
    protocol::{self, CompletionFormat},
};
use super::protocol::adif;
use tokio::time::Instant;

#[tokio::test(start_paused = true)]
async fn pairing_window_and_inflight_identity() -> anyhow::Result<()> {
    let bytes=adif(b"<CALL:5>W7HEX<QSO_DATE:8>20261103<TIME_ON:6>123456<FREQ:6>14.074<MODE:3>FT8<STATION_CALLSIGN:5>W7ABC<EOR>");
    let notice = protocol::decode(&bytes, "127.0.0.1:1".parse()?)?.unwrap();
    let context = super::context()?;
    let mut pairs = Pairing::default();
    let start = Instant::now();
    let Admission::New(key) = pairs.admit(notice.clone(), start) else {
        panic!("new")
    };
    pairs.attach_context(&key, context)?;
    assert!(
        pairs
            .take_ready(start + std::time::Duration::from_millis(99))
            .is_empty()
    );
    assert!(matches!(
        pairs.admit(notice.clone(), start + std::time::Duration::from_millis(80)),
        Admission::Joined(_)
    ));
    let ready = pairs.take_ready(start + std::time::Duration::from_millis(100));
    assert_eq!(ready.len(), 1);
    let ready = ready.into_iter().next().unwrap();
    assert_eq!(ready.format, Some(CompletionFormat::Adif));
    assert!(matches!(
        pairs.admit(notice.clone(), start),
        Admission::Suppressed
    ));
    pairs.settle(&key, true);
    assert!(matches!(pairs.admit(notice, start), Admission::Suppressed));
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn pairing_orders_fallback_and_latest_replacement() -> anyhow::Result<()> {
    let bytes=adif(b"<CALL:5>W7HEX<QSO_DATE:8>20261103<TIME_ON:6>123456<FREQ:6>14.074<MODE:3>FT8<STATION_CALLSIGN:5>W7ABC<EOR>");
    let adif = protocol::decode(&bytes, "127.0.0.1:1".parse()?)?.unwrap();
    let mut structured = adif.clone();
    structured.format = CompletionFormat::Structured;
    for first_adif in [false, true] {
        let mut pairs = Pairing::default();
        let now = Instant::now();
        let (first, second) = if first_adif {
            (adif.clone(), structured.clone())
        } else {
            (structured.clone(), adif.clone())
        };
        let Admission::New(key) = pairs.admit(first, now) else {
            panic!("new")
        };
        pairs.attach_context(&key, super::context()?)?;
        pairs.admit(second, now + std::time::Duration::from_millis(20));
        let ready = pairs.take_ready(now + std::time::Duration::from_millis(20));
        assert_eq!(ready.len(), 1);
        assert_eq!(ready.first().unwrap().format, Some(CompletionFormat::Adif));
        pairs.settle(&key, true);
        let mut next = adif.clone();
        next.identity.start = clacks_model::scalar::UtcSecond::from_unix_seconds(999);
        let Admission::New(next_key) = pairs.admit(next.clone(), now) else {
            panic!("new")
        };
        assert!(matches!(
            pairs.admit(adif.clone(), now),
            Admission::Suppressed
        ));
        pairs.attach_context(&next_key, super::context()?)?;
        pairs.take_ready(now + std::time::Duration::from_millis(100));
        pairs.settle(&next_key, true);
        assert!(matches!(pairs.admit(adif.clone(), now), Admission::New(_)));
    }
    // An ADIF record unable to supply own identity cannot defeat a usable type 5.
    let mut invalid = adif.clone();
    invalid.facts.values.remove("STATION_CALLSIGN");
    let mut context = super::context()?;
    context.operating = Default::default();
    let mut pairs = Pairing::default();
    let now = Instant::now();
    let Admission::New(key) = pairs.admit(invalid, now) else {
        panic!("new")
    };
    pairs.attach_context(&key, context)?;
    pairs.admit(structured, now);
    assert!(pairs.take_ready(now).is_empty());
    assert_eq!(
        pairs
            .take_ready(now + std::time::Duration::from_millis(100))
            .first()
            .unwrap()
            .format,
        Some(CompletionFormat::Structured)
    );
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn slow_context_read_does_not_extend_pairing_window() -> anyhow::Result<()> {
    let bytes=adif(b"<CALL:5>W7HEX<QSO_DATE:8>20261103<TIME_ON:6>123456<FREQ:6>14.074<MODE:3>FT8<STATION_CALLSIGN:5>W7ABC<EOR>");
    let adif = protocol::decode(&bytes, "127.0.0.1:1".parse()?)?.unwrap();
    let mut structured = adif.clone();
    structured.format = CompletionFormat::Structured;
    let mut pairs = Pairing::default();
    let now = Instant::now();
    let Admission::New(key) = pairs.admit(structured, now) else {
        panic!("new")
    };
    // Nothing was readable at the deadline; the local context is still pending.
    assert!(
        pairs
            .take_ready(now + std::time::Duration::from_millis(100))
            .is_empty()
    );
    pairs.admit(adif, now + std::time::Duration::from_millis(200));
    pairs.attach_context(&key, super::context()?)?;
    assert_eq!(
        pairs
            .take_ready(now + std::time::Duration::from_millis(250))
            .first()
            .unwrap()
            .format,
        Some(CompletionFormat::Structured)
    );
    Ok(())
}
