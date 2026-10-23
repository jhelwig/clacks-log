use std::{fs, path::Path, process::Command};

#[test]
fn checks_both_datasets_without_external_tools_or_writing_files() -> anyhow::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let paths = [
        "lib/clacks-model/data/lotw-11.34.json",
        "lib/clacks-lotw/data/signatures-11.34.json",
    ];
    let before: Vec<_> = paths
        .iter()
        .map(|p| fs::read(root.join(p)))
        .collect::<Result<_, _>>()?;
    let output = Command::new(env!("CARGO_BIN_EXE_clacks-lotw-data"))
        .arg("--check")
        .env("PATH", "")
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for (path, original) in paths.iter().zip(before) {
        assert_eq!(fs::read(root.join(path))?, original);
    }
    Ok(())
}

#[test]
fn rejects_unknown_actions_and_options() -> anyhow::Result<()> {
    for arguments in [
        vec!["--unknown"],
        vec!["other"],
        vec!["--check", "--unknown"],
        vec!["--check", "extra"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_clacks-lotw-data"))
            .args(arguments)
            .env("PATH", "")
            .output()?;
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
    }
    Ok(())
}

#[test]
fn help_describes_the_dedicated_tool_and_available_actions() -> anyhow::Result<()> {
    for arguments in [vec![], vec!["--help"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_clacks-lotw-data"))
            .args(arguments)
            .env("PATH", "")
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let help = String::from_utf8(output.stdout)?;
        for expected in ["clacks-lotw-data", "--check", "--write", "--help", "LoTW"] {
            assert!(help.contains(expected), "{help}");
        }
    }
    Ok(())
}
