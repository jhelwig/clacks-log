use super::*;

#[test]
fn allocation_changes_only_the_application_version() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manifest = directory.path().join("Cargo.toml");
    let original = "[package]\nname = \"clacks-log\"\nversion = \"0.1.0\" # desktop\n\n[dependencies]\nclacks-desktop = { path = \"../../lib/clacks-desktop\" }\n";
    let document = original.parse()?;

    write_version(&manifest, document, ReleaseVersion::parse("26.11.1")?)?;

    assert_eq!(
        std::fs::read_to_string(&manifest)?,
        original.replacen("version = \"0.1.0\"", "version = \"26.11.1\"", 1)
    );
    Ok(())
}
