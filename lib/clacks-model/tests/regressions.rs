use crate::support;

use clacks_model::contact::Qso;
use clacks_model::ids::QsoId;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn empty_present_aggregates_are_rejected_as_saved_contacts() -> TestResult {
    let base = support::qso(support::id::<QsoId>(1));
    for path in [
        "/own_station/location",
        "/contacted_station/location",
        "/own_context/operator",
        "/contacted_operator",
    ] {
        let mut input = serde_json::to_value(&base)?;
        *input.pointer_mut(path).unwrap() = serde_json::json!({});
        assert!(
            serde_json::from_value::<Qso>(input).is_err(),
            "accepted empty {path}"
        );
    }
    Ok(())
}

#[test]
fn empty_receive_override_is_rejected_at_input_boundary() -> TestResult {
    let base = support::qso(support::id::<QsoId>(1));
    let mut json = serde_json::to_value(&base)?;
    json["receive_override"] = serde_json::json!({"frequency":null,"band":null});
    assert!(serde_json::from_value::<Qso>(json).is_err());
    Ok(())
}
