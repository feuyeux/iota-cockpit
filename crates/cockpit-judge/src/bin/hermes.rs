use cockpit_judge::JudgeBackend;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    cockpit_judge::run_for_backend(JudgeBackend::Hermes).await
}
