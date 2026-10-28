use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = clacks_server::config::ServerArgs::parse();
    clacks_server::telemetry::init()?;
    let config = clacks_server::config::ServerConfig::load(&arguments.config)?;
    let state = clacks_server::runtime::ServerState::connect(config).await?;
    let options = leptos::config::get_configuration(None)?.leptos_options;
    let pages = clacks_web::routes(state.clone(), options);
    clacks_server::runtime::serve(state, pages).await
}
