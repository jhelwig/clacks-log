#[tokio::main]
async fn main() -> anyhow::Result<()> {
    clacks_server::maintenance::cli::run().await
}
