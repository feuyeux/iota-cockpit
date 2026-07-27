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
            let config_text = fs::read_to_string(&global_config).map_err(|error| {
                AcpAdapterError::Turn(format!(
                    "failed to read Hermes config {}: {error}",
                    global_config.display()
                ))
            })?;
            let isolated_config_path = profile.join("config.yaml");
            write_isolated_hermes_config(&config_text, &isolated_config_path)?;
        }
        let global_env = parent.join(".env");
        if global_env.is_file() {
            let _ = fs::copy(&global_env, profile.join(".env"));
        }
    }

    Ok(())
}

/// The toolsets this isolated Cockpit profile allows, expressed as an
/// allowlist rather than the previous approach of appending one disabled
/// entry to whatever `disabled_toolsets` already contained. `acp_toolsets`
/// takes precedence in Hermes' own config schema, so setting it directly to
/// an empty allowlist plus disabling the ACP toolset is the intended way to
/// express "no bundled skills, ACP-only surface" — not a side effect of
/// string-patching an unrelated key.
const COCKPIT_ACP_TOOLSET_ALLOWLIST: &[&str] = &[];
const COCKPIT_DISABLED_TOOLSET: &str = "hermes-acp";

/// Rewrites `config_text` (the global Hermes config) into the isolated
/// per-profile config Cockpit uses, and writes it to `destination`.
///
/// SECURITY/CORRECTNESS (result.md C-08 / AC15.1, AC15.2): the previous
/// implementation used `String::replacen("  disabled_toolsets: []", ...)`,
/// a brittle substring match that silently did nothing (no error, no
/// isolation applied) whenever the global config's actual formatting
/// differed even slightly — different indentation, `disabled_toolsets`
/// already containing entries, or any reformatting by a newer Hermes
/// version. A cockpit deployment could then run with the *global* toolset
/// surface instead of the isolated one, with no visible failure. This
/// version parses the config as structured YAML, sets the exact fields the
/// isolation requires, and re-parses what it wrote to assert the fields
/// actually landed — refusing to proceed (returning an error rather than
/// silently continuing) if that assertion fails.
fn write_isolated_hermes_config(
    config_text: &str,
    destination: &Path,
) -> Result<(), AcpAdapterError> {
    let mut document: serde_yaml::Value = serde_yaml::from_str(config_text).map_err(|error| {
        AcpAdapterError::Turn(format!("failed to parse Hermes config as YAML: {error}"))
    })?;

    let mapping = document.as_mapping_mut().ok_or_else(|| {
        AcpAdapterError::Turn("Hermes config root is not a YAML mapping".to_string())
    })?;

    let acp_toolsets_key = serde_yaml::Value::String("acp_toolsets".to_string());
    let acp_toolsets_value = serde_yaml::Value::Sequence(
        COCKPIT_ACP_TOOLSET_ALLOWLIST
            .iter()
            .map(|entry| serde_yaml::Value::String(entry.to_string()))
            .collect(),
    );
    mapping.insert(acp_toolsets_key, acp_toolsets_value);

    let disabled_toolsets_key = serde_yaml::Value::String("disabled_toolsets".to_string());
    let mut disabled: Vec<String> = mapping
        .get(&disabled_toolsets_key)
        .and_then(|value| value.as_sequence())
        .map(|sequence| {
            sequence
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if !disabled
        .iter()
        .any(|entry| entry == COCKPIT_DISABLED_TOOLSET)
    {
        disabled.push(COCKPIT_DISABLED_TOOLSET.to_string());
    }
    mapping.insert(
        disabled_toolsets_key,
        serde_yaml::Value::Sequence(
            disabled
                .into_iter()
                .map(serde_yaml::Value::String)
                .collect(),
        ),
    );

    let rendered = serde_yaml::to_string(&document).map_err(|error| {
        AcpAdapterError::Turn(format!(
            "failed to serialize isolated Hermes config: {error}"
        ))
    })?;
    fs::write(destination, &rendered).map_err(|error| {
        AcpAdapterError::Turn(format!(
            "failed to write isolated Hermes config {}: {error}",
            destination.display()
        ))
    })?;

    // AC15.2: re-parse what was just written and assert the allowlist is
    // exactly what was intended, rather than trusting the write succeeded
    // just because no I/O error occurred.
    assert_isolated_hermes_config(destination)?;

    Ok(())
}

/// Re-reads and re-parses `path`, asserting `acp_toolsets` matches
/// [`COCKPIT_ACP_TOOLSET_ALLOWLIST`] exactly and `disabled_toolsets`
/// contains [`COCKPIT_DISABLED_TOOLSET`]. Returns an error (never panics)
/// if the on-disk content does not match what
/// [`write_isolated_hermes_config`] intended to write, so a caller can
/// refuse to start a live backend against an unverified isolation config
/// (AC15.3) instead of silently trusting an unread file.
fn assert_isolated_hermes_config(path: &Path) -> Result<(), AcpAdapterError> {
    let written = fs::read_to_string(path).map_err(|error| {
        AcpAdapterError::Turn(format!(
            "failed to re-read isolated Hermes config {} for verification: {error}",
            path.display()
        ))
    })?;
    let document: serde_yaml::Value = serde_yaml::from_str(&written).map_err(|error| {
        AcpAdapterError::Turn(format!(
            "isolated Hermes config {} failed to re-parse after writing: {error}",
            path.display()
        ))
    })?;

    let acp_toolsets = document
        .get("acp_toolsets")
        .and_then(|value| value.as_sequence())
        .map(|sequence| {
            sequence
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let expected: Vec<String> = COCKPIT_ACP_TOOLSET_ALLOWLIST
        .iter()
        .map(|entry| entry.to_string())
        .collect();
    if acp_toolsets != expected {
        return Err(AcpAdapterError::Turn(format!(
            "isolated Hermes config {} does not match the expected acp_toolsets allowlist \
             after writing (expected {expected:?}, found {acp_toolsets:?}) — refusing to \
             proceed with an unverified isolation boundary",
            path.display()
        )));
    }

    let disabled_toolsets = document
        .get("disabled_toolsets")
        .and_then(|value| value.as_sequence())
        .map(|sequence| {
            sequence
                .iter()
                .any(|entry| entry.as_str() == Some(COCKPIT_DISABLED_TOOLSET))
        })
        .unwrap_or(false);
    if !disabled_toolsets {
        return Err(AcpAdapterError::Turn(format!(
            "isolated Hermes config {} does not disable '{COCKPIT_DISABLED_TOOLSET}' after \
             writing — refusing to proceed with an unverified isolation boundary",
            path.display()
        )));
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
