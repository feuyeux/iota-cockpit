use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use iota_core::config::{
    BackendConfig, BackendContextConfig, CommandConfig, ContextEngineBackendConfig,
    ContextEngineConfig, ContextInjection, NimiaConfig,
};

use super::AcpAdapterError;

#[derive(Debug, Clone)]
pub struct AcpAdapterConfig {
    pub backend: String,
    pub cwd: PathBuf,
    pub timeout_ms: u64,
    /// Executable that serves `mcp-bridge --state <path>` over stdio. `None`
    /// keeps the legacy text tool transport for deterministic/offline callers.
    pub native_mcp_bridge_command: Option<PathBuf>,
    pub native_mcp_state_path: Option<PathBuf>,
    /// Whether the configured bridge is exposed to the ACP backend as native
    /// MCP. Disabling it retains the bridge metadata for local skill routing
    /// while using the compatible textual tool protocol.
    pub native_mcp_transport: bool,
}

impl Default for AcpAdapterConfig {
    fn default() -> Self {
        Self {
            backend: "hermes".to_string(),
            cwd: PathBuf::from("."),
            // Hermes initializes its ACP tool surface before the first prompt;
            // a 20-second end-to-end budget can expire before `session/new`
            // has completed on a cold start.
            timeout_ms: 60_000,
            native_mcp_bridge_command: None,
            native_mcp_state_path: None,
            native_mcp_transport: true,
        }
    }
}

/// Cockpit owns the ACP transport command. Requiring a global iota-core YAML
/// backend section turns a local desktop dependency into a runtime failure.
/// Authentication remains in Hermes' own configured home directory.
fn hermes_acp_command() -> String {
    if let Some(command) = std::env::var_os("COCKPIT_HERMES_BIN") {
        return PathBuf::from(command).to_string_lossy().to_string();
    }
    #[cfg(windows)]
    let local_bin = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|root| {
            root.join("hermes")
                .join("hermes-agent")
                .join("venv")
                .join("Scripts")
                .join("hermes.exe")
        });

    #[cfg(not(windows))]
    let local_bin = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| hermes_path_in(&home));

    local_bin
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("hermes"))
        .to_string_lossy()
        .to_string()
}

#[cfg(any(not(windows), test))]
pub(super) fn hermes_path_in(home: &Path) -> PathBuf {
    home.join(".local").join("bin").join("hermes")
}

pub(super) fn cockpit_hermes_profile_home() -> PathBuf {
    if let Some(path) = std::env::var_os("COCKPIT_HERMES_HOME").filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }

    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .map(|home| home.join("AppData").join("Local"))
        })
        .unwrap_or_else(|| PathBuf::from("AppData").join("Local"))
        .join("hermes");

    #[cfg(not(windows))]
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".hermes");

    root.join("profiles").join("iota-cockpit")
}

pub(super) fn ensure_cockpit_hermes_profile() -> Result<(), AcpAdapterError> {
    let profile = cockpit_hermes_profile_home();
    fs::create_dir_all(profile.join("skills")).map_err(|error| {
        AcpAdapterError::Turn(format!(
            "failed to create isolated Hermes profile at {}: {error}",
            profile.display()
        ))
    })?;

    let marker = profile.join(".no-bundled-skills");
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker)
    {
        Ok(mut file) => file
            .write_all(b"Cockpit ACP profile: do not seed unrelated Hermes skills.\n")
            .map_err(|error| {
                AcpAdapterError::Turn(format!(
                    "failed to initialize isolated Hermes profile marker {}: {error}",
                    marker.display()
                ))
            })?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(AcpAdapterError::Turn(format!(
                "failed to initialize isolated Hermes profile marker {}: {error}",
                marker.display()
            )));
        }
    }

    if let Some(parent) = profile.parent().and_then(|p| p.parent()) {
        let global_config = parent.join("config.yaml");
        if global_config.is_file() {
            let config = fs::read_to_string(&global_config).map_err(|error| {
                AcpAdapterError::Turn(format!(
                    "failed to read Hermes config {}: {error}",
                    global_config.display()
                ))
            })?;
            let config = config.replacen(
                "  disabled_toolsets: []",
                "  acp_toolsets: []\n  disabled_toolsets:\n    - hermes-acp",
                1,
            );
            fs::write(profile.join("config.yaml"), config).map_err(|error| {
                AcpAdapterError::Turn(format!(
                    "failed to write isolated Hermes config {}: {error}",
                    profile.display()
                ))
            })?;
        }
        let global_env = parent.join(".env");
        if global_env.is_file() {
            let _ = fs::copy(&global_env, profile.join(".env"));
        }
    }

    Ok(())
}

pub(super) fn cockpit_acp_config(adapter: &AcpAdapterConfig) -> NimiaConfig {
    let native_mcp = adapter
        .native_mcp_bridge_command
        .as_ref()
        .zip(adapter.native_mcp_state_path.as_ref());
    let context_engine = match native_mcp {
        Some((command, state_path)) => ContextEngineConfig {
            enabled: true,
            injection: ContextInjection::Mcp,
            mcp: Some(CommandConfig {
                command: command.to_string_lossy().to_string(),
                args: vec![
                    "mcp-bridge".to_string(),
                    "--state".to_string(),
                    state_path.to_string_lossy().to_string(),
                ],
            }),
            fun: Some(CommandConfig {
                command: String::new(),
                args: Vec::new(),
            }),
            ..ContextEngineConfig::default()
        },
        None => ContextEngineConfig {
            enabled: false,
            ..ContextEngineConfig::default()
        },
    };
    NimiaConfig {
        hermes: Some(BackendConfig {
            enabled: true,
            home: Some(cockpit_hermes_profile_home().to_string_lossy().to_string()),
            acp: Some(CommandConfig {
                command: hermes_acp_command(),
                args: vec!["acp".to_string()],
            }),
            ..BackendConfig::default()
        }),
        context_engine: Some(context_engine),
        context_engine_backend: Some(ContextEngineBackendConfig {
            hermes: Some(BackendContextConfig {
                mcp_session_new: Some(native_mcp.is_some() && adapter.native_mcp_transport),
                always_send_empty_mcp_servers: true,
                override_home: true,
                ..BackendContextConfig::default()
            }),
            ..ContextEngineBackendConfig::default()
        }),
        ..NimiaConfig::default()
    }
}
