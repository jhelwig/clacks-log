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
    /// Install release-managed database migrations.
    Database {
        #[command(subcommand)]
        command: DatabaseCommand,
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
