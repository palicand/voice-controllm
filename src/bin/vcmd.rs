#[tokio::main]
async fn main() -> anyhow::Result<()> {
    voice_controllm_daemon::run().await
}
