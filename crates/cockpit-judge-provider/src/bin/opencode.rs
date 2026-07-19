use iota_core::AcpBackend;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    cockpit_judge_provider::run_for_backend(AcpBackend::OpenCode).await
}
