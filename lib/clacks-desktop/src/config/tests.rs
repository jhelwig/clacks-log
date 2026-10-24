//! Configuration precedence and safe-error regression cases.

use super::*;
use clap::Parser;

fn environment(cwd: std::path::PathBuf) -> Environment {
    Environment {
        directories: crate::workspace::AppDirectories {
            config: cwd.join("config"),
            data: cwd.join("data"),
        },
        cwd,
        values: Default::default(),
    }
}

#[test]
fn absent_default_config_uses_managed_paths_and_bounded_defaults() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let env = environment(directory.path().to_path_buf());
    let config = resolve(Cli::try_parse_from(["clacks-log"])?, &env)?;
    assert!(!config.telemetry.enabled);
    assert_eq!(config.logs.filter, "info");
    assert_eq!(config.logs.retained_files, 7);
    assert_eq!(config.logs.buffered_lines, 8192);
    assert!(!config.logs.console);
    Ok(())
}

#[test]
fn explicit_missing_config_is_an_error() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let env = environment(directory.path().to_path_buf());
    assert!(
        resolve(
            Cli::try_parse_from(["clacks-log", "--config", "absent.toml"])?,
            &env
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn command_environment_file_precedence_and_explicit_false_are_preserved() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    std::fs::write(
        directory.path().join("config.toml"),
        "workspace = 'account'\n[logs]\nfilter = 'debug'\nconsole = true\n[telemetry]\nenabled = true\nendpoint = 'https://file.invalid'\n",
    )?;
    let mut env = environment(directory.path().to_path_buf());
    env.values.insert("RUST_LOG".into(), "warn".into());
    env.values.insert(
        "OTEL_EXPORTER_OTLP_ENDPOINT".into(),
        "https://env.invalid".into(),
    );
    env.values.insert("OTEL_SDK_DISABLED".into(), "true".into());
    let config = resolve(
        Cli::try_parse_from([
            "clacks-log",
            "--config",
            "config.toml",
            "--console",
            "false",
            "--log-filter",
            "trace",
            "--otlp",
            "true",
        ])?,
        &env,
    )?;
    assert_eq!(config.logs.filter, "trace");
    assert!(!config.logs.console);
    assert!(config.telemetry.enabled);
    assert_eq!(config.workspace, directory.path().join("account"));
    assert_eq!(
        config.telemetry.traces.endpoint,
        "https://env.invalid/v1/traces"
    );
    Ok(())
}

#[test]
fn endpoints_do_not_implicitly_enable_export() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let mut env = environment(directory.path().to_path_buf());
    env.values.insert(
        "OTEL_EXPORTER_OTLP_ENDPOINT".into(),
        "https://env.invalid".into(),
    );
    env.values
        .insert("OTEL_SDK_DISABLED".into(), "false".into());
    assert!(
        !resolve(Cli::try_parse_from(["clacks-log"])?, &env)?
            .telemetry
            .enabled
    );
    Ok(())
}

#[test]
fn invalid_config_does_not_echo_secrets() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let env = environment(directory.path().to_path_buf());
    for body in [
        "[telemetry]\nheaders = { authorization = 'SECRET_SENTINEL' }\nbad_key = 5",
        "[telemetry]\nheaders = { authorization = 'SECRET_SENTINEL'\n",
        "[telemetry]\nendpoint = 'https://user:SECRET_SENTINEL@invalid/'\n",
    ] {
        std::fs::write(directory.path().join("config.toml"), body)?;
        let error = resolve(
            Cli::try_parse_from(["clacks-log", "--config", "config.toml"])?,
            &env,
        )
        .err()
        .expect("invalid config rejected");
        assert!(!format!("{error:#?}").contains("SECRET_SENTINEL"));
    }
    Ok(())
}

#[test]
fn shadowed_invalid_environment_does_not_override_cli() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let mut env = environment(directory.path().to_path_buf());
    env.values
        .insert("OTEL_SDK_DISABLED".into(), "not-a-boolean".into());
    env.values
        .insert("OTEL_EXPORTER_OTLP_TIMEOUT".into(), "not-a-timeout".into());
    let config = resolve(
        Cli::try_parse_from(["clacks-log", "--otlp", "true", "--otlp-timeout-ms", "10"])?,
        &env,
    )?;
    assert!(config.telemetry.enabled);
    assert_eq!(
        config.telemetry.traces.timeout,
        std::time::Duration::from_millis(10)
    );
    Ok(())
}

#[test]
fn signal_specificity_is_subordinate_to_configuration_tier() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let mut env = environment(directory.path().to_path_buf());
    env.values.insert(
        "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT".into(),
        "https://env.invalid/specific".into(),
    );
    env.values.insert(
        "OTEL_EXPORTER_OTLP_HEADERS".into(),
        "authorization=Bearer%20a+b".into(),
    );
    let config = resolve(
        Cli::try_parse_from(["clacks-log", "--otlp-endpoint", "https://cli.invalid/base"])?,
        &env,
    )?;
    assert_eq!(
        config.telemetry.traces.endpoint,
        "https://cli.invalid/base/v1/traces"
    );
    assert_eq!(
        config
            .telemetry
            .traces
            .headers
            .get("authorization")
            .map(String::as_str),
        Some("Bearer a+b")
    );
    Ok(())
}

#[test]
fn relative_file_paths_use_the_file_directory() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let nested = directory.path().join("nested");
    std::fs::create_dir(&nested)?;
    std::fs::write(
        nested.join("settings.toml"),
        "workspace='data'\n[logs]\ndirectory='diagnostics'\n",
    )?;
    let env = environment(directory.path().to_path_buf());
    let config = resolve(
        Cli::try_parse_from(["clacks-log", "--config", "nested/settings.toml"])?,
        &env,
    )?;
    assert_eq!(config.workspace, nested.join("data"));
    assert_eq!(config.logs.directory, nested.join("diagnostics"));
    let overridden = resolve(
        Cli::try_parse_from([
            "clacks-log",
            "--config",
            "nested/settings.toml",
            "--workspace",
            "elsewhere",
        ])?,
        &env,
    )?;
    assert_eq!(overridden.workspace, directory.path().join("elsewhere"));
    assert_eq!(overridden.logs.directory, nested.join("diagnostics"));
    Ok(())
}

#[test]
fn invalid_filters_and_nonpositive_capacities_are_rejected() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let env = environment(directory.path().to_path_buf());
    for arguments in [
        ["clacks-log", "--log-filter", "[broken"],
        ["clacks-log", "--log-retained-files", "0"],
        ["clacks-log", "--log-buffered-lines", "0"],
    ] {
        assert!(resolve(Cli::try_parse_from(arguments)?, &env).is_err());
    }
    Ok(())
}

#[test]
fn configuration_error_reports_the_actual_line() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let env = environment(directory.path().to_path_buf());
    std::fs::write(
        directory.path().join("config.toml"),
        "[logs]\nretained_files = 'bad'\n",
    )?;
    let error = resolve(
        Cli::try_parse_from(["clacks-log", "--config", "config.toml"])?,
        &env,
    )
    .err()
    .expect("invalid type rejected");
    assert!(error.to_string().contains("line 2"), "{error}");
    Ok(())
}

#[test]
fn unsupported_transport_and_invalid_header_values_are_safe_errors() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let mut env = environment(directory.path().to_path_buf());
    env.values
        .insert("OTEL_EXPORTER_OTLP_PROTOCOL".into(), "grpc".into());
    assert!(resolve(Cli::try_parse_from(["clacks-log"])?, &env).is_err());
    env.values.remove("OTEL_EXPORTER_OTLP_PROTOCOL");
    env.values.insert(
        "OTEL_EXPORTER_OTLP_HEADERS".into(),
        "authorization=SECRET_SENTINEL%0D%0A".into(),
    );
    let error = resolve(Cli::try_parse_from(["clacks-log"])?, &env)
        .err()
        .expect("invalid header rejected");
    assert!(!format!("{error:#?}").contains("SECRET_SENTINEL"));
    Ok(())
}
