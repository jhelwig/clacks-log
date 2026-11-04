//! Production native desktop startup and composition for Clacks Log.

mod application;
mod config;
mod credentials;
mod directory;
mod instance;
mod logbooks;
mod logging;
mod menus;
mod operating;
mod review;
mod session;
mod synchronization;
mod telemetry;
mod ui;
mod workspace;
mod ws_suite;

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

pub(crate) mod enrollment;

#[cfg(test)]
mod test_support;
