mod evaluation_commands;
mod runner_commands;

use evaluation_commands::EvaluationState;
use runner_commands::RunnerState;
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let token = format!(
        "cockpit-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let root = workspace_root(app);
            let history_root = app
                .path()
                .app_data_dir()?
                .join("evaluation-history");
            let evaluation = EvaluationState::new(
                &root,
                root.join("evaluations").join("private"),
                history_root,
            )
            .map_err(std::io::Error::other)?;
            let state = RunnerState::new(token, root);
            let heartbeat_state = state.clone();
            std::thread::spawn(move || heartbeat_state.run_heartbeat_loop());
            app.manage(state);
            app.manage(evaluation);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            runner_commands::connect_runner,
            runner_commands::validate_scenario,
            runner_commands::create_live_simulation_run,
            runner_commands::start_simulation,
            runner_commands::pause_simulation,
            runner_commands::step_live_simulation,
            runner_commands::stop_simulation,
            runner_commands::resume_simulation,
            runner_commands::resume_live_simulation,
            runner_commands::approve_action,
            runner_commands::reject_action,
            runner_commands::cancel_agent_turn,
            runner_commands::cancel_live_turn,
            runner_commands::set_approval_required,
            runner_commands::start_replay,
            runner_commands::diff_recordings,
            runner_commands::get_simulation_events,
            runner_commands::get_simulation_snapshot,
            evaluation_commands::evaluate_run,
            evaluation_commands::list_evaluation_reports,
        ])
        .run(tauri::generate_context!())
        .expect("error while running cockpit desktop");
}
