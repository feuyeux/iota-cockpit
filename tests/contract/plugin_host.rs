use std::{fs, path::PathBuf};

use cockpit_plugin::{
    PLUGIN_API_VERSION, PluginHost, PluginManifest, PluginPermission, PluginPolicy, StateDiff,
};
use cockpit_scenario::load_scenario;
use cockpit_simulation_core::Simulation;
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
            entity_id: "cabin".to_string(),
            component_path: "environment.visibility".to_string(),
            value: json!(0.5),
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
                entity_id: "engine-1".to_string(),
                component_path: "environment.smokeDensity".to_string(),
                value: json!(0.5),
                expected_state_version: simulation.snapshot.version,
            },
        )
        .expect_err("out-of-scope diff must fail");
    assert!(error.to_string().contains("outside plugin write scope"));
    let _ = fs::remove_dir_all(directory);
    let _ = fs::remove_dir_all(valid_directory);
}
