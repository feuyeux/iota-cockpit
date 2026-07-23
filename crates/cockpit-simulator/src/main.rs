use std::{
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "cockpit-simulator")]
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
    /// Benchmark serialized multi-human live ticks under strict and
    /// best-effort failure semantics using a deterministic backend shim.
    BenchLive {
        scenario: PathBuf,
        #[arg(long, default_value_t = 4)]
        human_count: usize,
        #[arg(long, default_value_t = 20)]
        ticks: u64,
        #[arg(long, default_value_t = 25)]
        backend_delay_ms: u64,
        #[arg(long)]
        failing_human_id: Option<String>,
    },
    #[command(hide = true)]
    McpBridge {
        #[arg(long)]
        state: PathBuf,
    },
    Serve {
        /// Address to bind. Defaults to an OS-assigned loopback port
        /// (`127.0.0.1:0`) rather than a fixed port, so multiple sidecar
        /// instances never collide and a fixed port cannot be pre-bound by
        /// another process to intercept connections (result.md C-02 /
        /// AC6.3). The actual bound address is printed to stdout as
        /// `SIMULATOR_READY <addr>` for the parent process to read back.
        #[arg(long, default_value = "127.0.0.1:0")]
        bind: String,
        /// Session token clients must present on every IPC request. The
        /// desktop parent supplies it through an inherited anonymous stdin
        /// pipe, so the secret is absent from argv and environment. This flag
        /// remains only for explicit standalone/test compatibility and should
        /// not be used for desktop sidecars.
        #[arg(long)]
        session_token: Option<String>,
        /// Allow binding to a non-loopback address. Off by default
        /// (result.md C-06 / AC13.1): this sidecar's IPC protocol has no
        /// TLS and authenticates with a plaintext shared session token, so
        /// exposing it to the network requires an explicit, visible
        /// operator opt-in.
        #[arg(long)]
        allow_remote: bool,
        /// Optional SQLite recording database. When set, the served process
        /// persists committed ticks so it can recover after a real restart.
        #[arg(long)]
        recording_db: Option<String>,
        #[arg(long, requires = "rule_policy_public_key_base64")]
        rule_policy_bundle: Option<PathBuf>,
        #[arg(long, requires = "rule_policy_bundle")]
        rule_policy_public_key_base64: Option<String>,
    },
    Validate {
        scenario: PathBuf,
    },
    Run {
        scenario: PathBuf,
        #[arg(long, default_value_t = 80)]
        ticks: u64,
        /// Write the complete immutable Recording JSON for an external evaluator.
        #[arg(long)]
        recording_output: Option<PathBuf>,
    },
    RunLive {
        scenario: PathBuf,
        #[arg(long, default_value_t = 80)]
        ticks: u64,
        #[arg(long, default_value_t = 2_000)]
        timeout_ms: u64,
        /// Commit successful human turns when another scheduled human fails.
        #[arg(long)]
        best_effort: bool,
        /// Write the complete immutable Recording JSON for an external evaluator.
        #[arg(long)]
        recording_output: Option<PathBuf>,
    },
    /// Generate a new Ed25519 policy signing key.
    PolicyKeygen {
        #[arg(long)]
        private_key: PathBuf,
    },
    /// Sign the exact manifest.json bytes in a policy bundle.
    PolicySign {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        private_key: PathBuf,
    },
    /// Revoke a policy in the signed manifest and append an audit record.
    PolicyRevoke {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        policy_id: String,
        #[arg(long)]
        private_key: PathBuf,
        #[arg(long)]
        audit_log: Option<PathBuf>,
    },
}

fn write_private_key(path: &Path, value: &str) -> anyhow::Result<()> {
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut file = std::fs::File::create(&temp)
        .with_context(|| format!("failed to create private key {}", temp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(value.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    std::fs::rename(&temp, path)
        .with_context(|| format!("failed to publish private key {}", path.display()))?;
    Ok(())
}

fn sign_policy_manifest(bundle: &Path, private_key: &Path) -> anyhow::Result<String> {
    let manifest_path = bundle.join("manifest.json");
    let manifest = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let private = std::fs::read_to_string(private_key)
        .with_context(|| format!("failed to read private key {}", private_key.display()))?;
    let signature = cockpit_agent::RulePolicyBundle::sign_manifest(&private, &manifest)
        .map_err(anyhow::Error::msg)?;
    let signature_path = bundle.join("manifest.sig");
    let temp = signature_path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temp, format!("{signature}\n"))?;
    std::fs::rename(&temp, &signature_path)?;
    Ok(signature)
}

fn write_recording(
    path: Option<PathBuf>,
    recording: &cockpit_recording::Recording,
) -> anyhow::Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    let bytes = cockpit_recording::serialize_redacted_recording(recording)?;
    std::fs::write(&path, bytes)
        .with_context(|| format!("failed to write recording {}", path.display()))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cockpit_agent::initialize_external_telemetry().map_err(anyhow::Error::msg)?;
    let cli = Cli::parse();
    match cli.command {
        Command::McpBridge { state } => {
            cockpit_agent::native_mcp::run_stdio(state).map_err(anyhow::Error::msg)?;
        }
        Command::Bench {
            scenario,
            ticks,
            active_entities,
            events_per_minute,
        } => {
            let report =
                cockpit_simulator::benchmark::run(cockpit_simulator::benchmark::BenchmarkConfig {
                    scenario_path: scenario.display().to_string(),
                    ticks,
                    active_entities,
                    events_per_minute,
                })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::BenchLive {
            scenario,
            human_count,
            ticks,
            backend_delay_ms,
            failing_human_id,
        } => {
            let report = cockpit_simulator::benchmark::run_live(
                cockpit_simulator::benchmark::LiveBenchmarkConfig {
                    scenario_path: scenario.display().to_string(),
                    human_count,
                    ticks,
                    backend_delay_ms,
                    failing_human_id,
                },
            )
            .await;
            println!("{}", serde_json::to_string_pretty(&report?)?);
        }
        Command::Serve {
            bind,
            session_token,
            recording_db,
            rule_policy_bundle,
            rule_policy_public_key_base64,
            allow_remote,
        } => {
            let session_token = match session_token {
                Some(token) => token,
                None => {
                    use std::io::BufRead;
                    let mut token = String::new();
                    std::io::stdin()
                        .lock()
                        .read_line(&mut token)
                        .context("failed to read session token from inherited stdin pipe")?;
                    let token = token.trim().to_string();
                    anyhow::ensure!(
                        !token.is_empty(),
                        "session token is required through inherited stdin or --session-token"
                    );
                    token
                }
            };
            cockpit_simulator::server::guard_bind_addr(&bind, allow_remote)
                .with_context(|| format!("refusing to bind {bind}"))?;
            if let (Some(bundle_path), Some(public_key)) =
                (rule_policy_bundle, rule_policy_public_key_base64)
            {
                let bundle =
                    cockpit_agent::RulePolicyBundle::discover_base64(bundle_path, &public_key)
                        .map_err(anyhow::Error::msg)?;
                cockpit_simulator::server::serve_persistent_with_policy_bundle(
                    &bind,
                    session_token,
                    recording_db.as_deref(),
                    bundle,
                )
                .await
            } else {
                cockpit_simulator::server::serve_persistent(
                    &bind,
                    session_token,
                    recording_db.as_deref(),
                )
                .await
            }
            .with_context(|| format!("failed to serve simulator on {bind}"))?;
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
        Command::Run {
            scenario,
            ticks,
            recording_output,
        } => {
            let scenario = cockpit_scenario::load_scenario(&scenario)
                .with_context(|| format!("failed to load {}", scenario.display()))?;
            let recording = cockpit_recording::run_rule_agent_recording(
                "simulator-run-1",
                scenario.clone(),
                ticks,
            )?;
            write_recording(recording_output, &recording)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "runId": recording.run_id,
                    "scenarioHash": recording.scenario_hash,
                    "ticks": recording.ticks.len(),
                    "finalSnapshotHash": recording.final_snapshot_hash(),
                    "evaluation": {
                        "status": "pending",
                        "evaluator": "cockpit-evaluator"
                    }
                }))?
            );
        }
        Command::RunLive {
            scenario,
            ticks,
            timeout_ms,
            best_effort,
            recording_output,
        } => {
            let report = cockpit_simulator::run_live(cockpit_simulator::LiveRunConfig {
                scenario_path: scenario.display().to_string(),
                ticks,
                timeout_ms,
                tick_mode: if best_effort {
                    cockpit_agent::LiveTickMode::BestEffort
                } else {
                    cockpit_agent::LiveTickMode::Strict
                },
            })
            .await
            .with_context(|| format!("failed to run live agent on {}", scenario.display()))?;
            write_recording(recording_output, &report.recording)?;
            let run_failed = report.error.is_some();
            println!("{}", serde_json::to_string_pretty(&report)?);
            if run_failed {
                anyhow::bail!(
                    "live run aborted by a mandatory backend failure: {}",
                    report.error.unwrap_or_default()
                );
            }
        }
        Command::PolicyKeygen { private_key } => {
            let (private, public) = cockpit_agent::RulePolicyBundle::generate_signing_key()
                .map_err(anyhow::Error::msg)?;
            write_private_key(&private_key, &private)?;
            println!(
                "{}",
                serde_json::json!({
                    "privateKeyPath": private_key,
                    "publicKeyBase64": public,
                })
            );
        }
        Command::PolicySign {
            bundle,
            private_key,
        } => {
            let signature = sign_policy_manifest(&bundle, &private_key)?;
            let private = std::fs::read_to_string(&private_key)?;
            let public = cockpit_agent::RulePolicyBundle::public_key_from_private(&private)
                .map_err(anyhow::Error::msg)?;
            println!(
                "{}",
                serde_json::json!({ "publicKeyBase64": public, "signature": signature })
            );
        }
        Command::PolicyRevoke {
            bundle,
            policy_id,
            private_key,
            audit_log,
        } => {
            let manifest_path = bundle.join("manifest.json");
            let manifest_bytes = std::fs::read(&manifest_path)?;
            let mut manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)?;
            let policies = manifest
                .get("policies")
                .and_then(serde_json::Value::as_object)
                .ok_or_else(|| anyhow::anyhow!("manifest policies must be an object"))?;
            if !policies.contains_key(&policy_id) {
                return Err(anyhow::anyhow!(
                    "policy '{policy_id}' is not in the manifest"
                ));
            }
            let revoked = manifest
                .as_object_mut()
                .expect("manifest object")
                .entry("revokedPolicies")
                .or_insert_with(|| serde_json::json!([]));
            let list = revoked
                .as_array_mut()
                .ok_or_else(|| anyhow::anyhow!("manifest revokedPolicies must be an array"))?;
            if !list
                .iter()
                .any(|item| item.as_str() == Some(policy_id.as_str()))
            {
                list.push(serde_json::Value::String(policy_id.clone()));
                list.sort_by_key(|item| item.as_str().unwrap_or_default().to_string());
            }
            let updated = serde_json::to_vec_pretty(&manifest)?;
            let temp = manifest_path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
            std::fs::write(&temp, &updated)?;
            std::fs::rename(&temp, &manifest_path)?;
            let signature = sign_policy_manifest(&bundle, &private_key)?;
            use sha2::{Digest, Sha256};
            let audit = serde_json::json!({
                "action": "revoke",
                "policyId": policy_id,
                "manifestHash": format!("sha256:{:x}", Sha256::digest(&updated)),
                "signature": signature,
                "timestampMs": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis(),
            });
            let audit_path = audit_log.unwrap_or_else(|| bundle.join("policy-revocations.jsonl"));
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&audit_path)?;
            writeln!(file, "{}", serde_json::to_string(&audit)?)?;
            file.sync_all()?;
            println!(
                "{}",
                serde_json::json!({ "policyId": policy_id, "auditPath": audit_path })
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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
    fn every_bundled_public_scenario_runs_without_embedded_scoring() {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

        for relative_path in BUNDLED_SCENARIOS {
            let path = workspace_root.join(relative_path);
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert!(!source.contains("evaluation:"), "{}", path.display());
            assert!(!source.contains("deadlineTick"), "{}", path.display());
            let scenario = cockpit_scenario::load_scenario(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert!(!scenario.public_goals.is_empty(), "{}", path.display());
            cockpit_recording::run_rule_agent_recording(
                format!("simulator-public-{}", scenario.id),
                scenario.clone(),
                scenario.max_ticks,
            )
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        }
    }
}
