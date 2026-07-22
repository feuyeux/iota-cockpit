use std::{fs, path::PathBuf, time::Duration};

use cockpit_plugin::{
    PLUGIN_API_VERSION, PluginExecutor, PluginFailurePolicy, PluginHost, PluginManifest,
    PluginPermission, PluginPolicy, PluginStatus, PluginTickOutcome, ProcessPluginExecutor,
    StateDiff,
};
use cockpit_scenario::load_scenario;
use cockpit_world::{Simulation, StatePatch};
use serde_json::json;
use sha2::{Digest, Sha256};

fn manifest_bytes(mut manifest: PluginManifest) -> Vec<u8> {
    manifest.hash.clear();
    let canonical = serde_json::to_vec(&manifest).expect("manifest serializes");
    let mut hasher = Sha256::new();
    hasher.update(canonical);
    manifest.hash = format!("sha256:{:x}", hasher.finalize());
    serde_json::to_vec(&manifest).expect("manifest serializes")
}

fn plugin_dir(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("cockpit-plugin-{name}"));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("plugin directory creates");
    directory
}

fn base_manifest(permissions: Vec<PluginPermission>) -> PluginManifest {
    PluginManifest {
        id: "smoke-plugin".to_string(),
        version: "1.0.0".to_string(),
        api_contract: PLUGIN_API_VERSION,
        permissions,
        schema: json!({"kind": "smoke"}),
        hash: String::new(),
        signature: None,
        command: None,
        filesystem_read_paths: Vec::new(),
    }
}

fn write_policy() -> PluginPolicy {
    PluginPolicy {
        allowed_permissions: [PluginPermission::WorldRead, PluginPermission::WorldWrite]
            .into_iter()
            .collect(),
        ..PluginPolicy::default()
    }
}

struct StaticExecutor {
    output: Result<Vec<StateDiff>, String>,
}

impl PluginExecutor for StaticExecutor {
    fn tick(&mut self, _snapshot: &cockpit_world::WorldSnapshot) -> Result<Vec<StateDiff>, String> {
        self.output.clone()
    }
}

struct SlowExecutor {
    sleep_ms: u64,
}

impl PluginExecutor for SlowExecutor {
    fn tick(&mut self, _snapshot: &cockpit_world::WorldSnapshot) -> Result<Vec<StateDiff>, String> {
        std::thread::sleep(std::time::Duration::from_millis(self.sleep_ms));
        Ok(Vec::new())
    }
}

#[test]
fn valid_manifest_loads_and_state_diff_is_scoped() {
    let directory = plugin_dir("valid");
    fs::write(
        directory.join("plugin.json"),
        manifest_bytes(base_manifest(vec![PluginPermission::WorldWrite])),
    )
    .expect("manifest writes");
    let mut host = PluginHost::default();
    let failures = host.discover(&directory, &write_policy());
    assert!(failures.is_empty(), "{failures:?}");

    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("plugin-run", scenario);
    host.validate_state_diff(
        &simulation.snapshot,
        &StateDiff {
            plugin_id: "smoke-plugin".to_string(),
            patch: StatePatch::CabinVisibility { value: 0.5 },
            expected_state_version: simulation.snapshot.version,
        },
    )
    .expect("valid diff");
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn plugin_hash_permission_and_diff_scope_fail_closed() {
    let directory = plugin_dir("invalid");
    let mut manifest = base_manifest(vec![PluginPermission::Network]);
    manifest.hash = "sha256:wrong".to_string();
    fs::write(
        directory.join("plugin.json"),
        serde_json::to_vec(&manifest).expect("manifest serializes"),
    )
    .expect("manifest writes");
    let mut host = PluginHost::default();
    let failures = host.discover(&directory, &PluginPolicy::default());
    assert_eq!(failures.len(), 1);
    assert!(failures[0].reason.contains("permission") || failures[0].reason.contains("hash"));

    let valid_directory = plugin_dir("scope");
    fs::write(
        valid_directory.join("plugin.json"),
        manifest_bytes(base_manifest(vec![PluginPermission::WorldWrite])),
    )
    .expect("manifest writes");
    let mut host = PluginHost::default();
    host.discover(&valid_directory, &write_policy());
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("plugin-run", scenario);
    let error = host
        .validate_state_diff(
            &simulation.snapshot,
            &StateDiff {
                plugin_id: "smoke-plugin".to_string(),
                patch: StatePatch::CabinSmokeDensity { value: 99.0 },
                expected_state_version: simulation.snapshot.version,
            },
        )
        .expect_err("out-of-scope diff must fail");
    assert!(error.to_string().contains("outside plugin write scope"));
    let _ = fs::remove_dir_all(directory);
    let _ = fs::remove_dir_all(valid_directory);
}

#[test]
fn plugin_tick_output_is_validated_and_failures_disable_the_plugin() {
    let directory = plugin_dir("tick");
    fs::write(
        directory.join("plugin.json"),
        manifest_bytes(base_manifest(vec![PluginPermission::WorldWrite])),
    )
    .expect("manifest writes");
    let mut host = PluginHost::default();
    let policy = write_policy();
    assert!(host.discover(&directory, &policy).is_empty());
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("plugin-run", scenario);

    let mut valid = StaticExecutor {
        output: Ok(vec![StateDiff {
            plugin_id: "smoke-plugin".to_string(),
            patch: StatePatch::CabinVisibility { value: 0.5 },
            expected_state_version: simulation.snapshot.version,
        }]),
    };
    assert!(matches!(
        host.run_tick("smoke-plugin", &simulation.snapshot, &mut valid, &policy),
        PluginTickOutcome::Accepted(diffs) if diffs.len() == 1
    ));

    let mut invalid = StaticExecutor {
        output: Ok(vec![StateDiff {
            plugin_id: "smoke-plugin".to_string(),
            patch: StatePatch::EngineHealth { value: 99.0 },
            expected_state_version: simulation.snapshot.version,
        }]),
    };
    let outcome = host.run_tick("smoke-plugin", &simulation.snapshot, &mut invalid, &policy);
    assert!(matches!(
        outcome,
        PluginTickOutcome::Failed(ref failure)
            if failure.decision == PluginFailurePolicy::DisablePlugin
    ));
    assert_eq!(
        host.get("smoke-plugin").map(|plugin| &plugin.status),
        Some(&PluginStatus::Disabled)
    );
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn plugin_tick_over_budget_fails_closed() {
    let directory = plugin_dir("budget");
    fs::write(
        directory.join("plugin.json"),
        manifest_bytes(base_manifest(vec![PluginPermission::WorldWrite])),
    )
    .expect("manifest writes");
    let mut host = PluginHost::default();
    let policy = PluginPolicy {
        allowed_permissions: [PluginPermission::WorldRead, PluginPermission::WorldWrite]
            .into_iter()
            .collect(),
        tick_budget_ms: Some(5),
        ..PluginPolicy::default()
    };
    assert!(host.discover(&directory, &policy).is_empty());
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("plugin-run", scenario);

    let mut slow = SlowExecutor { sleep_ms: 40 };
    let outcome = host.run_tick("smoke-plugin", &simulation.snapshot, &mut slow, &policy);
    assert!(matches!(
        outcome,
        PluginTickOutcome::Failed(ref failure) if failure.reason.contains("budget")
    ));
    assert_eq!(
        host.get("smoke-plugin").map(|plugin| &plugin.status),
        Some(&PluginStatus::Disabled),
        "an over-budget plugin is disabled by the failure policy"
    );
    let _ = fs::remove_dir_all(directory);
}

#[cfg(unix)]
#[test]
fn process_plugin_deadline_kills_a_hung_plugin() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("process-plugin-run", scenario);
    let mut executor = ProcessPluginExecutor::new(
        "sh",
        vec!["-c".to_string(), "while :; do :; done".to_string()],
        Duration::from_millis(20),
    )
    .with_permissions([PluginPermission::ChildProcess]);
    let started = std::time::Instant::now();
    let error = executor
        .tick(&simulation.snapshot)
        .expect_err("hung process must be terminated");
    assert!(error.contains("deadline"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[cfg(unix)]
#[test]
fn process_plugin_deadline_covers_a_child_that_never_reads_stdin() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("process-plugin-stdin-run", scenario);
    let mut snapshot = simulation.snapshot.clone();
    let human = snapshot
        .humans
        .first()
        .expect("scenario has a human")
        .clone();
    // Exceed the typical pipe capacity so a synchronous stdin write would
    // block before it could observe the executor deadline.
    snapshot.humans = vec![human; 8_192];
    assert!(
        serde_json::to_vec(&snapshot)
            .expect("snapshot serializes")
            .len()
            > 1_048_576
    );
    let mut executor = ProcessPluginExecutor::new(
        "sh",
        vec!["-c".to_string(), "sleep 1".to_string()],
        Duration::from_millis(20),
    )
    .with_permissions([PluginPermission::ChildProcess]);

    let started = std::time::Instant::now();
    let error = executor
        .tick(&snapshot)
        .expect_err("a child that never reads stdin must still time out");
    assert!(error.contains("deadline"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(
        executor
            .take_execution_evidence()
            .is_some_and(|evidence| evidence.timed_out && evidence.terminated_process_group)
    );
}

#[cfg(unix)]
#[test]
fn process_plugin_deadline_kills_its_background_child_processes() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("process-group-plugin-run", scenario);
    let marker = std::env::temp_dir().join(format!("cockpit-plugin-escape-{}", std::process::id()));
    let _ = fs::remove_file(&marker);
    let marker = marker.display().to_string().replace('\'', "'\\\"'\\\"'");
    let script = format!("(sleep 0.2; : > '{marker}') & while :; do :; done");
    let mut executor = ProcessPluginExecutor::new(
        "sh",
        vec!["-c".to_string(), script],
        Duration::from_millis(20),
    )
    .with_permissions([PluginPermission::ChildProcess]);

    executor
        .tick(&simulation.snapshot)
        .expect_err("deadline must terminate the plugin group");
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !std::path::Path::new(&marker).exists(),
        "a background child must not survive the plugin deadline"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_seccomp_denies_child_creation_without_permission() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("linux-seccomp-child-run", scenario);
    let mut executor = ProcessPluginExecutor::new(
        "sh",
        vec!["-c".to_string(), "sleep 1".to_string()],
        Duration::from_millis(200),
    );

    let error = executor
        .tick(&simulation.snapshot)
        .expect_err("fork must be rejected without ChildProcess permission");
    assert!(error.contains("exited"), "{error}");
    assert!(
        executor
            .take_execution_evidence()
            .is_some_and(|evidence| !evidence.timed_out)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_landlock_denies_unlisted_file_reads() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("linux-landlock-read-run", scenario);
    let mut executor = ProcessPluginExecutor::new(
        "sh",
        vec!["-c".to_string(), "cat /etc/hosts".to_string()],
        Duration::from_millis(200),
    );

    let error = executor
        .tick(&simulation.snapshot)
        .expect_err("Landlock must reject an unlisted file read");
    assert!(error.contains("exited"), "{error}");
    assert!(
        executor
            .take_execution_evidence()
            .is_some_and(|evidence| !evidence.timed_out)
    );
}

#[cfg(target_os = "macos")]
#[test]
fn macos_process_plugin_sandbox_denies_undeclared_file_writes() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("sandboxed-plugin-run", scenario);
    let marker = std::env::temp_dir().join(format!(
        "cockpit-plugin-sandbox-write-{}",
        uuid::Uuid::new_v4()
    ));
    let marker = marker.display().to_string().replace('\'', "'\\\"'\\\"'");
    let script = format!("cat >/dev/null; : > '{marker}'");
    let mut executor = ProcessPluginExecutor::new(
        "sh",
        vec!["-c".to_string(), script],
        Duration::from_millis(100),
    );

    executor
        .tick(&simulation.snapshot)
        .expect_err("sandbox must reject a filesystem write");
    assert!(
        !std::path::Path::new(&marker).exists(),
        "the sandboxed plugin must not create its marker"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn macos_process_plugin_filesystem_read_requires_an_explicit_allow_list() {
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("sandboxed-plugin-read-run", scenario);
    let marker = std::env::temp_dir().join(format!(
        "cockpit-plugin-sandbox-read-{}",
        uuid::Uuid::new_v4()
    ));
    fs::write(&marker, b"plugin input").expect("marker writes");
    let escaped = marker.display().to_string().replace('\'', "'\\\"'\\\"'");
    let script = format!("cat '{escaped}' >/dev/null || exit 42; printf '[]'");
    let mut denied = ProcessPluginExecutor::new(
        "sh",
        vec!["-c".to_string(), script.clone()],
        Duration::from_millis(100),
    )
    .with_permissions([PluginPermission::ChildProcess]);
    denied
        .tick(&simulation.snapshot)
        .expect_err("undeclared user path must not be readable");

    let mut allowed = ProcessPluginExecutor::new(
        "sh",
        vec!["-c".to_string(), script],
        Duration::from_millis(100),
    )
    .with_permissions([
        PluginPermission::ChildProcess,
        PluginPermission::FilesystemRead,
    ])
    .with_filesystem_read_paths([marker.parent().expect("marker parent")]);
    assert_eq!(
        allowed
            .tick(&simulation.snapshot)
            .expect("allow-listed read"),
        []
    );
    let _ = fs::remove_file(marker);
}

#[cfg(unix)]
#[test]
fn process_plugin_failure_carries_deadline_execution_evidence() {
    let directory = plugin_dir("process-evidence");
    fs::write(
        directory.join("plugin.json"),
        manifest_bytes(base_manifest(vec![PluginPermission::WorldWrite])),
    )
    .expect("manifest writes");
    let mut host = PluginHost::default();
    let policy = write_policy();
    assert!(host.discover(&directory, &policy).is_empty());
    let scenario = load_scenario("scenarios/smoke-in-cockpit.yaml").expect("scenario loads");
    let simulation = Simulation::new("process-evidence-run", scenario);
    let mut executor = ProcessPluginExecutor::new(
        "sh",
        vec!["-c".to_string(), "while :; do :; done".to_string()],
        Duration::from_millis(20),
    )
    .with_permissions([PluginPermission::ChildProcess]);

    let outcome = host.run_tick("smoke-plugin", &simulation.snapshot, &mut executor, &policy);
    assert!(matches!(
        outcome,
        PluginTickOutcome::Failed(ref failure)
            if failure.execution.as_ref().is_some_and(|execution|
                execution.timed_out && execution.terminated_process_group && execution.elapsed_ms >= 20)
    ));
    let _ = fs::remove_dir_all(directory);
}
