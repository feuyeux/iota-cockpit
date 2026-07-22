//! Serializable plugin execution failure evidence shared by tick records,
//! simulator IPC, and durable recordings.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginFailureRecord {
    pub plugin_id: String,
    pub version: String,
    pub reason: String,
    pub decision: String,
    #[serde(default)]
    pub execution: Option<PluginExecutionRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginExecutionRecord {
    pub elapsed_ms: u64,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub terminated_process_group: bool,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
}
