use super::*;

#[test]
fn independently_checks_signature_padding_and_rejects_changed_inputs() -> anyhow::Result<()> {
    let key = signing::key(1024, 11)?;
    let signature = signing::sign(&key, b"literal fixture input")?;
    signing::verify(&key.to_public_key(), b"literal fixture input", &signature)?;
    assert!(signing::verify(&key.to_public_key(), b"changed", &signature).is_err());
    let mut changed = signature;
    if let Some(first) = changed.first_mut() {
        *first ^= 1;
    }
    assert!(signing::verify(&key.to_public_key(), b"literal fixture input", &changed).is_err());
    Ok(())
}

#[test]
fn bounds_cli_actions_before_any_generation_or_writes() {
    assert!(run(["--unknown".into()]).is_err());
    assert!(run(["--check".into(), "extra".into()]).is_err());
}
