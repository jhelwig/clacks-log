//! Discoverable operator commands with operation-specific credentials.

use clacks_model::ids::AccountId;
use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};

/// Operator command arguments; nested commands load only their own inputs.
#[derive(Parser)]
#[command(name = "clacks-admin", about = "Clacks Log operator commands")]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

/// Supported operator domains, not a general task registry.
#[derive(Subcommand)]
enum Command {
    /// Create deployment encryption key files.
    Keys {
        #[command(subcommand)]
        command: KeysCommand,
    },

    /// Register a provider client for this deployment.
    Oidc {
        #[command(subcommand)]
        command: OidcCommand,
    },
    /// Run release database maintenance.
    Database {
        #[command(subcommand)]
        command: DatabaseCommand,
    },
    /// Assign existing accounts the server-administrator role.
    Role {
        #[command(subcommand)]
        command: RoleCommand,
    },
    /// Recover an existing account using an operator-verified identity.
    Identity {
        #[command(subcommand)]
        command: IdentityCommand,
    },
}

/// Deployment secret creation without database or provider credentials.
#[derive(Subcommand)]
enum KeysCommand {
    /// Generate a protected 32-byte wrapping key without overwriting an existing file.
    Generate {
        /// Destination in a private, existing secret directory.
        #[arg(long)]
        output: PathBuf,
    },
}

/// Provider deployment setup operations.

#[derive(Subcommand)]
enum OidcCommand {
    /// Reuse or create a protected deployment registration.
    Register {
        /// Deployment configuration file.
        #[arg(long)]
        config: PathBuf,
        /// Configured provider key.
        #[arg(long)]
        provider: String,
    },
}

/// Database release operations.
#[derive(Subcommand)]
enum DatabaseCommand {
    /// Apply migrations using a schema-changing database credential.
    Migrate {
        /// File containing the migration-role PostgreSQL connection URL.
        #[arg(long)]
        database_url_file: PathBuf,
    },
}

/// Existing account and maintenance connection for a role operation.
#[derive(Args)]
struct RoleTarget {
    /// Stable internal account ID, not a provider subject or email.
    #[arg(long)]
    account: AccountId,
    /// File containing the maintenance-role PostgreSQL connection URL.
    #[arg(long)]
    database_url_file: PathBuf,
}

/// Explicit role-assignment operations.
#[derive(Subcommand)]
enum RoleCommand {
    /// Grant the server-administrator role to an existing account.
    Grant(RoleTarget),
    /// Revoke the server-administrator role without changing login identities.
    Revoke(RoleTarget),
}

/// Exceptional operator-assisted identity operations.
#[derive(Subcommand)]
enum IdentityCommand {
    /// Add an externally verified identity without transferring or merging accounts.
    Recover {
        /// Existing stable Clacks account ID.
        #[arg(long)]
        account: AccountId,
        /// Exact verified provider issuer.
        #[arg(long)]
        issuer: String,
        /// Exact verified subject, without case folding or email inference.
        #[arg(long, value_parser = clap::builder::NonEmptyStringValueParser::new())]
        subject: String,
        /// File containing the maintenance-role PostgreSQL connection URL.
        #[arg(long)]
        database_url_file: PathBuf,
    },
}

/// Open only the explicitly selected operation's database connection.
async fn database(path: &Path) -> anyhow::Result<sqlx::PgPool> {
    Ok(sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&crate::config::secret(path)?)
        .await?)
}

/// Execute a role operation through the production store and maintenance grants.
async fn change_role(target: RoleTarget, enabled: bool) -> anyhow::Result<()> {
    let pool = database(&target.database_url_file).await?;
    let outcome = crate::store::AccountStore::new(pool.clone())
        .set_administrator(target.account, enabled)
        .await?;
    println!("{outcome:?}");
    pool.close().await;
    Ok(())
}

/// Run one operator operation without loading web-runtime credentials or starting HTTP.
pub async fn run() -> anyhow::Result<()> {
    let args = Arguments::parse();
    let telemetry = crate::telemetry::init(false)?;
    match args.command {
        Command::Keys {
            command: KeysCommand::Generate { output },
        } => {
            super::keys::generate(&output)?;
            println!("Wrapping key created at {}", output.display());
        }

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
            let pool = database(&database_url_file).await?;
            super::migrate::run(&pool).await?;
            pool.close().await;
        }
        Command::Role {
            command: RoleCommand::Grant(target),
        } => change_role(target, true).await?,
        Command::Role {
            command: RoleCommand::Revoke(target),
        } => change_role(target, false).await?,
        Command::Identity {
            command:
                IdentityCommand::Recover {
                    account,
                    issuer,
                    subject,
                    database_url_file,
                },
        } => {
            let pool = database(&database_url_file).await?;
            let outcome = crate::store::AccountStore::new(pool.clone())
                .recover_identity(
                    account,
                    crate::auth::oidc::VerifiedIdentity {
                        issuer: openidconnect::IssuerUrl::new(issuer)?.as_str().to_owned(),
                        subject,
                        display_name: None,
                    },
                )
                .await?;
            println!("{outcome:?}");
            pool.close().await;
        }
    }
    tokio::task::spawn_blocking(move || telemetry.shutdown()).await??;
    Ok(())
}
