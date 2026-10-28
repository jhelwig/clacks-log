//! Process observability composition for the production server executable.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Install structured operator logs once at process startup.
pub fn init() -> anyhow::Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        // The dependency's trace diagnostics contain middleware bearer identifiers.
        .add_directive("axum_session=off".parse()?);
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(std::io::stderr),
        )
        .try_init()
        .map_err(|error| anyhow::anyhow!("could not initialize operator logging: {error}"))?;
    Ok(())
}
