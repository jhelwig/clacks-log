//! Configuration precedence and safe-error regression cases.

use super::*;
use clap::Parser;

struct Inputs {
    cwd: PathBuf,
    values: ::config::Map<String, String>,
    directories: AppDirectories,
}

impl Inputs {
    fn context(&self) -> StartupContext {
        StartupContext {
            cwd: self.cwd.clone(),
            environment: ::config::Environment::default().source(Some(self.values.clone())),
            directories: AppDirectories {
                config: self.directories.config.clone(),
                data: self.directories.data.clone(),
            },
        }
    }
}

fn environment(cwd: PathBuf) -> Inputs {
    Inputs {
        directories: AppDirectories {
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

    let config = resolve(Cli::try_parse_from(["clacks-log"])?, &env.context())?;

    assert!(!config.telemetry.enabled);
    assert_eq!(config.logs.filter.to_string(), "info");
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
            &env.context()
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
            "--console=false",
            "--log-filter",
            "trace",
            "--otlp",
        ])?,
        &env.context(),
    )?;

    assert_eq!(config.logs.filter.to_string(), "trace");
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
        !resolve(Cli::try_parse_from(["clacks-log"])?, &env.context())?
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
            &env.context(),
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
        Cli::try_parse_from(["clacks-log", "--otlp", "--otlp-timeout-ms", "10"])?,
        &env.context(),
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
        &env.context(),
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
        &env.context(),
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
        &env.context(),
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
        assert!(resolve(Cli::try_parse_from(arguments)?, &env.context()).is_err());
    }
    Ok(())
}

#[test]
fn configuration_error_identifies_the_setting_without_its_value() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let env = environment(directory.path().to_path_buf());
    std::fs::write(
        directory.path().join("config.toml"),
        "[logs]\nretained_files = 'bad'\n",
    )?;

    let error = resolve(
        Cli::try_parse_from(["clacks-log", "--config", "config.toml"])?,
        &env.context(),
    )
    .err()
    .expect("invalid type rejected");

    assert!(error.to_string().contains("logs.retained_files"), "{error}");
    assert!(!format!("{error:#?}").contains("bad"));
    Ok(())
}

#[test]
fn unsupported_transport_and_invalid_header_values_are_safe_errors() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let mut env = environment(directory.path().to_path_buf());
    env.values
        .insert("OTEL_EXPORTER_OTLP_PROTOCOL".into(), "grpc".into());

    assert!(resolve(Cli::try_parse_from(["clacks-log"])?, &env.context()).is_err());
    env.values.remove("OTEL_EXPORTER_OTLP_PROTOCOL");
    env.values.insert(
        "OTEL_EXPORTER_OTLP_HEADERS".into(),
        "authorization=SECRET_SENTINEL%0D%0A".into(),
    );

    let error = resolve(Cli::try_parse_from(["clacks-log"])?, &env.context())
        .err()
        .expect("invalid header rejected");

    assert!(!format!("{error:#?}").contains("SECRET_SENTINEL"));
    Ok(())
}

#[test]
fn bare_output_flags_enable_reporting_and_keep_values_unambiguous() -> anyhow::Result<()> {
    use clap::CommandFactory;
    let directory = tempfile::tempdir()?;
    let env = environment(directory.path().to_path_buf());

    let config = resolve(
        Cli::try_parse_from(["clacks-log", "--otlp", "--console"])?,
        &env.context(),
    )?;

    assert!(config.telemetry.enabled);
    assert!(config.logs.console);
    let help = Cli::command().render_long_help().to_string();

    assert!(help.contains("<BOOL>"));
    assert!(!help.contains("<OTLP>"));
    assert!(!help.contains("<CONSOLE>"));
    Ok(())
}

#[test]
fn omitted_output_flags_preserve_file_settings_and_false_overrides_them() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let env = environment(directory.path().to_path_buf());
    std::fs::create_dir_all(&env.directories.config)?;
    std::fs::write(
        env.directories.config.join("config.toml"),
        "[logs]\nconsole = true\n[telemetry]\nenabled = true\n",
    )?;
    let arguments = Cli::try_parse_from(["clacks-log"])?;

    assert_eq!(arguments.otlp, None);
    assert_eq!(arguments.console, None);
    let inherited = resolve(arguments, &env.context())?;

    assert!(inherited.telemetry.enabled && inherited.logs.console);
    let disabled = resolve(
        Cli::try_parse_from(["clacks-log", "--otlp=false", "--console=false"])?,
        &env.context(),
    )?;

    assert!(!disabled.telemetry.enabled && !disabled.logs.console);
    Ok(())
}

#[test]
fn telemetry_source_order_preserves_endpoint_meaning_and_header_overrides() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let mut inputs = environment(directory.path().to_path_buf());
    std::fs::create_dir_all(&inputs.directories.config)?;
    std::fs::write(
        inputs.directories.config.join("config.toml"),
        r#"
[telemetry]
endpoint = "https://file.invalid/base"
headers = { Authorization = "file-token", x-common = "common" }
[telemetry.traces]
endpoint = "https://file.invalid/complete"
headers = { x-trace = "trace" }
"#,
    )?;
    inputs.values.insert(
        "OTEL_EXPORTER_OTLP_ENDPOINT".into(),
        "https://env.invalid/base".into(),
    );
    inputs.values.insert(
        "OTEL_EXPORTER_OTLP_HEADERS".into(),
        "authorization=env-token".into(),
    );

    let config = resolve(Cli::try_parse_from(["clacks-log"])?, &inputs.context())?;

    assert_eq!(
        config.telemetry.traces.endpoint,
        "https://env.invalid/base/v1/traces"
    );

    assert_eq!(
        config
            .telemetry
            .traces
            .headers
            .get("authorization")
            .map(String::as_str),
        Some("env-token")
    );

    assert_eq!(
        config
            .telemetry
            .traces
            .headers
            .get("x-trace")
            .map(String::as_str),
        Some("trace")
    );

    assert_eq!(
        config
            .telemetry
            .metrics
            .headers
            .get("x-common")
            .map(String::as_str),
        Some("common")
    );

    assert!(!config.telemetry.metrics.headers.contains_key("x-trace"));
    Ok(())
}

#[test]
fn selected_config_is_an_exact_file_and_unreadable_default_is_not_absence() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let inputs = environment(directory.path().to_path_buf());
    std::fs::write(
        directory.path().join("selected.toml.toml"),
        "workspace = 'unexpected'",
    )?;

    assert!(
        resolve(
            Cli::try_parse_from(["clacks-log", "--config", "selected.toml"])?,
            &inputs.context()
        )
        .is_err()
    );
    std::fs::create_dir_all(inputs.directories.config.join("config.toml"))?;

    assert!(resolve(Cli::try_parse_from(["clacks-log"])?, &inputs.context()).is_err());
    Ok(())
}
