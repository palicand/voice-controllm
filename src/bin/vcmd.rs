#[tokio::main]
async fn main() -> anyhow::Result<()> {
    vcm_daemon::run().await
}
