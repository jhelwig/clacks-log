use super::*;

#[test]
fn bounds_cli_actions_before_any_generation_or_writes() {
    assert!(run(["--unknown".into()]).is_err());
    assert!(run(["--check".into(), "extra".into()]).is_err());
}
