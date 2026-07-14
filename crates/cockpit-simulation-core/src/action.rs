use serde::{Deserialize, Serialize};

use crate::{id::AgentId, sensor::Observation};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Command {
    EngineShutdown,
    AlarmActivate,
    ClimateComfortRestore,
    WindshieldDefogActivate,
    FatigueInterventionActivate,
    ChildProtectionActivate,
    MedicalResponseActivate,
    PrivacyModeActivate,
    ChargingPlanAccept,
    AdasTakeoverAcknowledge,
    CyberSafeModeActivate,
}

impl Command {
    /// Every command in wire order, so callers (e.g. prompt construction) can
    /// enumerate the action surface without hardcoding a parallel list that
    /// silently drifts when a command is added.
    pub const ALL: [Command; 11] = [
        Command::EngineShutdown,
        Command::AlarmActivate,
        Command::ClimateComfortRestore,
        Command::WindshieldDefogActivate,
        Command::FatigueInterventionActivate,
        Command::ChildProtectionActivate,
        Command::MedicalResponseActivate,
        Command::PrivacyModeActivate,
        Command::ChargingPlanAccept,
        Command::AdasTakeoverAcknowledge,
        Command::CyberSafeModeActivate,
    ];

    /// Authoritative component paths an action can mutate during commit.
    /// These drive conflict arbitration before any shared state is changed.
    pub fn write_set(&self) -> &'static [&'static str] {
        match self {
            Self::EngineShutdown => &["engine-1.shutdown", "cabin.fireActive"],
            Self::AlarmActivate => &["alarm-1.active", "cabin.noiseDb"],
            Self::ClimateComfortRestore => &["climate.cooling", "cabin.temperatureC"],
            Self::WindshieldDefogActivate => &["climate.defog", "cabin.visibility"],
            Self::FatigueInterventionActivate
            | Self::PrivacyModeActivate
            | Self::AdasTakeoverAcknowledge
            | Self::CyberSafeModeActivate => &["driver-1.attention"],
            Self::ChildProtectionActivate => &[
                "occupant.childProtection",
                "cabin.temperatureC",
                "child-1.stress",
            ],
            Self::MedicalResponseActivate => &["occupant.medicalResponse", "patient-1.stress"],
            Self::ChargingPlanAccept => &["mobility.chargingRoute", "driver-1.stress"],
        }
    }
    pub fn from_wire_name(value: &str) -> Option<Self> {
        Some(match value {
            "engineShutdown" => Self::EngineShutdown,
            "alarmActivate" => Self::AlarmActivate,
            "climateComfortRestore" => Self::ClimateComfortRestore,
            "windshieldDefogActivate" => Self::WindshieldDefogActivate,
            "fatigueInterventionActivate" => Self::FatigueInterventionActivate,
            "childProtectionActivate" => Self::ChildProtectionActivate,
            "medicalResponseActivate" => Self::MedicalResponseActivate,
            "privacyModeActivate" => Self::PrivacyModeActivate,
            "chargingPlanAccept" => Self::ChargingPlanAccept,
            "adasTakeoverAcknowledge" => Self::AdasTakeoverAcknowledge,
            "cyberSafeModeActivate" => Self::CyberSafeModeActivate,
            _ => return None,
        })
    }

    pub fn wire_name(&self) -> &'static str {
        match self {
            Self::EngineShutdown => "engineShutdown",
            Self::AlarmActivate => "alarmActivate",
            Self::ClimateComfortRestore => "climateComfortRestore",
            Self::WindshieldDefogActivate => "windshieldDefogActivate",
            Self::FatigueInterventionActivate => "fatigueInterventionActivate",
            Self::ChildProtectionActivate => "childProtectionActivate",
            Self::MedicalResponseActivate => "medicalResponseActivate",
            Self::PrivacyModeActivate => "privacyModeActivate",
            Self::ChargingPlanAccept => "chargingPlanAccept",
            Self::AdasTakeoverAcknowledge => "adasTakeoverAcknowledge",
            Self::CyberSafeModeActivate => "cyberSafeModeActivate",
        }
    }

    pub fn capability_name(&self) -> &'static str {
        match self {
            Self::EngineShutdown => "engine.shutdown",
            Self::AlarmActivate => "alarm.activate",
            Self::ClimateComfortRestore => "climate.restoreComfort",
            Self::WindshieldDefogActivate => "visibility.activateDefog",
            Self::FatigueInterventionActivate => "driver.activateFatigueIntervention",
            Self::ChildProtectionActivate => "occupant.activateChildProtection",
            Self::MedicalResponseActivate => "health.activateMedicalResponse",
            Self::PrivacyModeActivate => "privacy.activateMode",
            Self::ChargingPlanAccept => "energy.acceptChargingPlan",
            Self::AdasTakeoverAcknowledge => "adas.acknowledgeTakeover",
            Self::CyberSafeModeActivate => "cybersecurity.enterSafeMode",
        }
    }

    pub fn target_id(&self) -> &'static str {
        match self {
            Self::EngineShutdown => "engine-1",
            Self::AlarmActivate => "alarm-1",
            Self::ClimateComfortRestore => "hvac-1",
            Self::WindshieldDefogActivate => "defogger-1",
            Self::FatigueInterventionActivate => "dms-1",
            Self::ChildProtectionActivate => "occupant-radar-1",
            Self::MedicalResponseActivate => "emergency-call-1",
            Self::PrivacyModeActivate => "voice-array-1",
            Self::ChargingPlanAccept => "navigation-1",
            Self::AdasTakeoverAcknowledge => "adas-controller-1",
            Self::CyberSafeModeActivate => "security-monitor-1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    CapabilityDenied,
    DeviceUnpowered,
    PreconditionFailed,
    #[serde(rename = "STATE_VERSION_CONFLICT")]
    VersionMismatch,
    ActionExpired,
    ActionConflict,
    UnknownTarget,
    ApprovalDenied,
    ActionCancelled,
}

impl ErrorCode {
    pub fn stable_code(&self) -> &'static str {
        match self {
            Self::CapabilityDenied => "CAPABILITY_DENIED",
            Self::DeviceUnpowered => "DEVICE_UNPOWERED",
            Self::PreconditionFailed => "PRECONDITION_FAILED",
            Self::VersionMismatch => "STATE_VERSION_CONFLICT",
            Self::ActionExpired => "ACTION_EXPIRED",
            Self::ActionConflict => "ACTION_CONFLICT",
            Self::UnknownTarget => "UNKNOWN_TARGET",
            Self::ApprovalDenied => "APPROVAL_DENIED",
            Self::ActionCancelled => "ACTION_CANCELLED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionStatus {
    Applied,
    Rejected,
    Superseded,
    PendingApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionRequest {
    pub request_id: String,
    pub agent_id: AgentId,
    pub target: String,
    pub command: Command,
    pub expected_state_version: u64,
    pub expires_at_tick: u64,
    #[serde(default)]
    pub correlation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub request: ActionRequest,
    pub status: ActionStatus,
    pub error_code: Option<ErrorCode>,
    pub run_id: String,
    pub tick: u64,
    pub correlation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentGrant {
    pub agent_id: AgentId,
    pub capabilities: Vec<String>,
}

impl AgentGrant {
    pub fn allows(&self, agent_id: &str, command: &Command) -> bool {
        self.agent_id == agent_id
            && self
                .capabilities
                .iter()
                .any(|capability| capability == command.capability_name())
    }
}

#[derive(Debug, Default)]
pub struct ScriptedAgent {
    action_sent: bool,
}

impl ScriptedAgent {
    pub fn next_actions(
        &mut self,
        observation: &Observation,
        state_version: u64,
    ) -> Vec<ActionRequest> {
        if self.action_sent
            || !observation
                .alerts
                .iter()
                .any(|alert| alert == "SmokeDetected")
        {
            return Vec::new();
        }

        self.action_sent = true;
        vec![ActionRequest {
            request_id: format!("{}-shutdown", observation.observation_id),
            agent_id: observation.agent_id.clone(),
            target: "engine-1".to_string(),
            command: Command::EngineShutdown,
            expected_state_version: state_version,
            expires_at_tick: observation.delivered_tick + 3,
            correlation_id: format!("{}-corr", observation.observation_id),
        }]
    }
}
