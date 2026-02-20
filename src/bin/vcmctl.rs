#[tokio::main]
async fn main() -> anyhow::Result<()> {
    vcmctl::run().await
}
