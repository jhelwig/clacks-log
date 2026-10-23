use std::process::Command;

#[test]
fn help_and_invalid_arguments_work_without_external_programs() -> anyhow::Result<()> {
    for arguments in [vec![], vec!["--help"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_clacks-lotw-fixtures"))
            .args(arguments)
            .env("PATH", "")
            .output()?;
        assert!(output.status.success());
        let help = String::from_utf8(output.stdout)?;
        for expected in ["clacks-lotw-fixtures", "--check", "--write", "--help"] {
            assert!(help.contains(expected), "{help}");
        }
    }
    for arguments in [vec!["--unknown"], vec!["--write", "extra"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_clacks-lotw-fixtures"))
            .args(arguments)
            .env("PATH", "")
            .output()?;
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
    }
    Ok(())
}
