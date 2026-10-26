//! Production native desktop startup and composition for Clacks Log.

mod application;
mod config;
mod instance;
mod logbooks;
mod operating;
mod session;
mod telemetry;
mod ui;
mod workspace;

/// Run the native desktop on the calling main thread using startup configuration.
///
/// Secondary launches activate the existing owner and return. Startup, native
/// window and cleanup failures are returned for local command-line diagnostics.
pub fn run() -> anyhow::Result<()> {
    use clap::Parser;
    let arguments = config::Cli::parse();
    let context = config::StartupContext::discover()?;
    let configuration = config::resolve(arguments, &context)?;
    application::run(configuration)
}

#[cfg(test)]
#[path = "logbooks/tests.rs"]
mod logbook_tests;
