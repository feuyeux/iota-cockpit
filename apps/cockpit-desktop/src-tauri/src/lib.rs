mod evaluation_commands;
mod simulator_commands;

use evaluation_commands::EvaluationState;
use simulator_commands::SimulatorState;
use std::path::PathBuf;
use tauri::Manager;

/// Return the directory that contains the packaged `scenarios/` and
/// `evaluations/` folders. In a development checkout, retain the current
/// workspace directory so the same relative paths continue to work.
fn workspace_root(app: &tauri::App) -> PathBuf {
    if let Ok(resources) = app.path().resource_dir()
        && resources.join("scenarios").is_dir()
    {
        return resources;
    }

    let development_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    if let Ok(development_root) = development_root.canonicalize()
        && development_root.join("scenarios").is_dir()
    {
        return development_root;
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Generates an unpredictable session token for authenticating IPC requests
/// to the cockpit-simulator sidecar, using the OS CSPRNG (32 random bytes,
/// hex-encoded) rather than a timestamp (result.md C-02 / AC6.1).
fn generate_session_token() -> String {
    use rand::TryRngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .expect("OS CSPRNG must be available to generate a session token");
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("cockpit-{hex}")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // SECURITY (result.md C-02): the sidecar session token must be
    // unpredictable. A token derived from `SystemTime::now().as_nanos()` is
    // a low-entropy value an attacker on the same host could plausibly
    // guess or narrow down (process start time is observable via `ps`/
    // `/proc`), which would let them forge IPC requests to the simulator
    // sidecar. Generate the token from the OS CSPRNG instead.
    let token = generate_session_token();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let root = workspace_root(app);
            let app_data_dir = app.path().app_data_dir()?;
            let history_root = app_data_dir.join("evaluation-history");
            let evaluation = EvaluationState::new(
                &root,
                root.join("evaluations").join("private"),
                history_root,
            )
            .map_err(std::io::Error::other)?;
            // SECURITY (result.md C-05 / AC12.1): recording databases go
            // under the Tauri app data directory (owner-only by OS/Tauri
            // convention) rather than the shared OS temp directory.
            let state = SimulatorState::new_with_recordings_dir(token, root, app_data_dir);
            let heartbeat_state = state.clone();
            std::thread::spawn(move || heartbeat_state.run_heartbeat_loop());
            app.manage(state);
            app.manage(evaluation);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            simulator_commands::connect_simulator,
            simulator_commands::validate_scenario,
            simulator_commands::create_simulation_run,
            simulator_commands::list_rule_policies,
            simulator_commands::select_rule_policy,
            simulator_commands::create_live_simulation_run,
            simulator_commands::start_simulation,
            simulator_commands::pause_simulation,
            simulator_commands::step_live_simulation,
            simulator_commands::step_simulation,
            simulator_commands::stop_simulation,
            simulator_commands::resume_simulation,
            simulator_commands::resume_live_simulation,
            simulator_commands::approve_action,
            simulator_commands::reject_action,
            simulator_commands::cancel_agent_turn,
            simulator_commands::cancel_live_turn,
            simulator_commands::set_approval_required,
            simulator_commands::start_replay,
            simulator_commands::diff_recordings,
            simulator_commands::get_simulation_events,
            simulator_commands::get_recorded_audit_events,
            simulator_commands::get_simulation_snapshot,
            evaluation_commands::evaluate_run,
            evaluation_commands::list_evaluation_reports,
        ])
        .run(tauri::generate_context!())
        .expect("error while running cockpit desktop");
}
