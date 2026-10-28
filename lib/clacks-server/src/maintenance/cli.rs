//! Discoverable operator commands with operation-specific credentials.

use clap::{Parser, Subcommand};

/// Operator command arguments; nested commands load only their own inputs.
#[derive(Parser)]
#[command(name = "clacks-admin", about = "Clacks Log operator commands")]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

/// Supported operator domains.
#[derive(Subcommand)]
enum Command {
    /// Register a configured identity provider for this deployment.
    Oidc {
        #[command(subcommand)]
        command: OidcCommand,
    },
    /// Install release-managed database migrations.
    Database {
        #[command(subcommand)]
        command: DatabaseCommand,
    },
}

/// Provider deployment setup operations.
#[derive(Subcommand)]
enum OidcCommand {
    /// Reuse or create a protected deployment registration.
    Register {
        /// Deployment configuration file.
        #[arg(long)]
        config: std::path::PathBuf,
        /// Configured provider key.
        #[arg(long)]
        provider: String,
    },
}

/// Database maintenance operations.
#[derive(Subcommand)]
enum DatabaseCommand {
    /// Apply migrations using a schema-changing database credential.
    Migrate {
        /// File containing the migration-role PostgreSQL connection URL.
        #[arg(long)]
        database_url_file: std::path::PathBuf,
    },
}

/// Run the chosen operator command without starting the web application.
pub async fn run() -> anyhow::Result<()> {
    let args = Arguments::parse();
    match args.command {
        Command::Oidc {
            command: OidcCommand::Register { config, provider },
        } => {
            let config = crate::config::ServerConfig::load(&config)?;
            let callback = config.callback(&provider)?;
            let provider = config
                .providers
                .get(&provider)
                .ok_or_else(|| anyhow::anyhow!("provider is not configured"))?;
            let outcome = super::registration::run(provider, &callback).await?;
            println!("{outcome:?}");
        }
        Command::Database {
            command: DatabaseCommand::Migrate { database_url_file },
        } => {
            let url = std::fs::read_to_string(database_url_file)?;
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(1)
                .connect(url.trim())
                .await?;
            super::migrate::run(&pool).await?;
            pool.close().await;
        }
    }
    Ok(())
}
