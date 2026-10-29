use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = clacks_server::config::ServerArgs::parse();

    let config = clacks_server::config::ServerConfig::load(&arguments.config)?;
    let telemetry = clacks_server::telemetry::init(config.otlp)?;
    let result = async {
        let state = clacks_server::runtime::ServerState::connect(config).await?;
        let options = leptos::config::get_configuration(None)?.leptos_options;
        let pages = clacks_web::routes(state.clone(), options);
        clacks_server::runtime::serve(state, pages).await
    }
    .await;
    let flushed = tokio::task::spawn_blocking(move || telemetry.shutdown()).await;
    if !matches!(flushed, Ok(Ok(()))) {
        let _local = opentelemetry::Context::enter_telemetry_suppressed_scope();
        tracing::warn!("Telemetry export incomplete during shutdown");
    }
    result
}
