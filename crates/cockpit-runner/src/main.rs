use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "cockpit-runner")]
#[command(about = "Validate and run deterministic cockpit simulation scenarios")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Bench {
        scenario: PathBuf,
        #[arg(long, default_value_t = 120)]
        ticks: u64,
        #[arg(long, default_value_t = 1000)]
        active_entities: u64,
        #[arg(long, default_value_t = 10000)]
        events_per_minute: u64,
    },
    Serve {
        #[arg(long, default_value = "127.0.0.1:47701")]
        bind: String,
        #[arg(long)]
        session_token: String,
        /// Optional SQLite recording database. When set, the served process
        /// persists committed ticks so it can recover after a real restart.
        #[arg(long)]
        recording_db: Option<String>,
    },
    Validate {
        scenario: PathBuf,
    },
    Run {
        scenario: PathBuf,
        #[arg(long, default_value_t = 80)]
        ticks: u64,
    },
    RunLive {
        scenario: PathBuf,
        #[arg(long, default_value_t = 80)]
        ticks: u64,
        #[arg(long, default_value_t = 2_000)]
        timeout_ms: u64,
    },
}

fn evaluate_recording(
    recording: &cockpit_recording::Recording,
    scenario: &cockpit_simulation_core::SimulationScenario,
) -> cockpit_evaluation::EvaluationResult {
    cockpit_evaluation::evaluate(
        recording,
        scenario.evaluation_rule_id.as_deref(),
        scenario.shutdown_deadline_ticks,
        &scenario.language,
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Bench {
            scenario,
            ticks,
            active_entities,
            events_per_minute,
        } => {
            let report =
                cockpit_runner::benchmark::run(cockpit_runner::benchmark::BenchmarkConfig {
                    scenario_path: scenario.display().to_string(),
                    ticks,
                    active_entities,
                    events_per_minute,
                })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Serve {
            bind,
            session_token,
            recording_db,
        } => {
            cockpit_runner::server::serve_persistent(&bind, session_token, recording_db.as_deref())
                .await
                .with_context(|| format!("failed to serve runner on {bind}"))?;
        }
        Command::Validate { scenario } => {
            let scenario = cockpit_scenario::load_scenario(&scenario)
                .with_context(|| format!("failed to validate {}", scenario.display()))?;
            println!(
                "{}",
                serde_json::json!({
                    "ok": true,
                    "scenarioId": scenario.id,
                    "scenarioHash": scenario.scenario_hash,
                    "schemaVersion": scenario.schema_version
                })
            );
        }
        Command::Run { scenario, ticks } => {
            let scenario = cockpit_scenario::load_scenario(&scenario)
                .with_context(|| format!("failed to load {}", scenario.display()))?;
            let recording = cockpit_recording::run_rule_agent_recording(
                "runner-run-1",
                scenario.clone(),
                ticks,
            )?;
            let evaluation = evaluate_recording(&recording, &scenario);
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "runId": recording.run_id,
                    "scenarioHash": recording.scenario_hash,
                    "ticks": recording.ticks.len(),
                    "finalSnapshotHash": recording.final_snapshot_hash(),
                    "evaluation": evaluation
                }))?
            );
        }
        Command::RunLive {
            scenario,
            ticks,
            timeout_ms,
        } => {
            let report = cockpit_runner::run_live(cockpit_runner::LiveRunConfig {
                scenario_path: scenario.display().to_string(),
                ticks,
                timeout_ms,
            })
            .await
            .with_context(|| format!("failed to run live agent on {}", scenario.display()))?;
            let run_failed = report.error.is_some();
            println!("{}", serde_json::to_string_pretty(&report)?);
            if run_failed {
                anyhow::bail!(
                    "live run aborted by a mandatory backend failure: {}",
                    report.error.unwrap_or_default()
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::evaluate_recording;

    const BUNDLED_SCENARIOS: &[&str] = &[
        "scenarios/smoke-in-cockpit.yaml",
        "scenarios/heatwave-thermal-comfort.yaml",
        "scenarios/winter-defog-visibility.yaml",
        "scenarios/driver-fatigue-guardian.yaml",
        "scenarios/child-left-behind.yaml",
        "scenarios/medical-emergency.yaml",
        "scenarios/voice-privacy-conflict.yaml",
        "scenarios/ev-range-anxiety.yaml",
        "scenarios/adas-takeover-construction.yaml",
        "scenarios/cybersecurity-anomalous-control.yaml",
    ];

    #[test]
    fn every_bundled_scenario_runs_and_passes_its_registered_evaluation() {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

        for relative_path in BUNDLED_SCENARIOS {
            let path = workspace_root.join(relative_path);
            let scenario = cockpit_scenario::load_scenario(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let recording = cockpit_recording::run_rule_agent_recording(
                format!("runner-evaluation-{}", scenario.id),
                scenario.clone(),
                scenario.shutdown_deadline_ticks + 1,
            )
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let evaluation = evaluate_recording(&recording, &scenario);

            assert!(
                evaluation.passed,
                "{}: {}",
                path.display(),
                evaluation.explanation
            );
        }
    }
}
