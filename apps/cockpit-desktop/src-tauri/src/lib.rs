mod runner_commands;

use runner_commands::RunnerState;

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
        .manage(RunnerState::new(token))
        .invoke_handler(tauri::generate_handler![
            runner_commands::connect_runner,
            runner_commands::validate_scenario,
            runner_commands::create_simulation_run,
            runner_commands::start_simulation,
            runner_commands::pause_simulation,
            runner_commands::step_simulation,
            runner_commands::stop_simulation,
            runner_commands::resume_simulation,
            runner_commands::approve_action,
            runner_commands::reject_action,
            runner_commands::cancel_agent_turn,
            runner_commands::set_approval_required,
            runner_commands::start_replay,
            runner_commands::get_simulation_events,
            runner_commands::get_simulation_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running cockpit desktop");
}
