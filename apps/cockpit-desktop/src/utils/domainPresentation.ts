import type { Locale } from "../i18n";

interface Presentation {
  "zh-CN": string;
  "en-US": string;
}

const eventLabels: Record<string, Presentation> = {
  SmokeDetected: { "zh-CN": "检测到烟雾", "en-US": "Smoke detected" },
  SmokeDensityChanged: { "zh-CN": "烟雾密度变化", "en-US": "Smoke density changed" },
  VisibilityChanged: { "zh-CN": "能见度变化", "en-US": "Visibility changed" },
  CabinTemperatureChanged: { "zh-CN": "舱温变化", "en-US": "Cabin temperature changed" },
  HumanStateDeltaApplied: { "zh-CN": "人物状态已更新", "en-US": "Occupant state updated" },
  HumanPhysiologyUpdated: { "zh-CN": "人物生理状态已更新", "en-US": "Occupant physiology updated" },
  VoiceRequestRaised: { "zh-CN": "乘员语音请求", "en-US": "Occupant voice request" },
  MessagePrivacyRiskRaised: { "zh-CN": "私信隐私风险", "en-US": "Private-message privacy risk" },
  DriverDistractionRaised: { "zh-CN": "驾驶分心风险", "en-US": "Driver distraction risk" },
  CabinSmokeEmergencyRaised: { "zh-CN": "座舱烟雾紧急情况", "en-US": "Cabin smoke emergency" },
  SmokeEmergencyEscalated: { "zh-CN": "烟雾风险升级", "en-US": "Smoke emergency escalated" },
  HeatComfortRiskRaised: { "zh-CN": "高温舒适风险", "en-US": "Heat comfort risk" },
  DriverHeatStrainRaised: { "zh-CN": "驾驶员热应激风险", "en-US": "Driver heat-strain risk" },
  WindshieldFogRiskRaised: { "zh-CN": "前风挡起雾风险", "en-US": "Windshield fog risk" },
  VisibilitySafetyRiskRaised: { "zh-CN": "视野安全风险", "en-US": "Visibility safety risk" },
  DriverFatigueSignalRaised: { "zh-CN": "驾驶员疲劳信号", "en-US": "Driver fatigue signal" },
  DriverFatigueEscalated: { "zh-CN": "驾驶员疲劳升级", "en-US": "Driver fatigue escalated" },
  ChildLeftBehindRaised: { "zh-CN": "儿童遗留风险", "en-US": "Child-left-behind risk" },
  ChildHeatExposureEscalated: { "zh-CN": "儿童热暴露升级", "en-US": "Child heat exposure escalated" },
  PassengerMedicalEmergencyRaised: { "zh-CN": "乘员医疗紧急情况", "en-US": "Passenger medical emergency" },
  MedicalCoordinationRiskRaised: { "zh-CN": "医疗协同风险", "en-US": "Medical coordination risk" },
  EvRangeRiskRaised: { "zh-CN": "电动车续航风险", "en-US": "EV range risk" },
  RangeComfortTradeoffRaised: { "zh-CN": "续航与舒适权衡", "en-US": "Range-comfort trade-off" },
  ConstructionTakeoverRaised: { "zh-CN": "施工区接管请求", "en-US": "Construction-zone takeover request" },
  TakeoverUrgencyRaised: { "zh-CN": "接管紧迫性升级", "en-US": "Takeover urgency escalated" },
  RemoteControlAnomalyRaised: { "zh-CN": "远程控制异常", "en-US": "Remote-control anomaly" },
  CyberContainmentRiskRaised: { "zh-CN": "网络安全控制风险", "en-US": "Cyber containment risk" },
  EngineFire: { "zh-CN": "动力系统起火", "en-US": "Engine fire" },
  EngineShutdown: { "zh-CN": "动力系统已关闭", "en-US": "Engine shutdown" },
  ActionApplied: { "zh-CN": "动作已提交", "en-US": "Action applied" },
  ActionRejected: { "zh-CN": "动作被拒绝", "en-US": "Action rejected" },
  InfluenceApplied: { "zh-CN": "外部影响已应用", "en-US": "External influence applied" },
  StateDiffApplied: { "zh-CN": "状态差异已提交", "en-US": "State difference applied" },
  ThermalComfortRestored: { "zh-CN": "热舒适已恢复", "en-US": "Thermal comfort restored" },
  WindshieldVisibilityRestored: { "zh-CN": "前风挡视野已恢复", "en-US": "Windshield visibility restored" },
  DriverAttentionRestored: { "zh-CN": "驾驶员注意力已恢复", "en-US": "Driver attention restored" },
  ChildProtectionActivated: { "zh-CN": "儿童保护已激活", "en-US": "Child protection activated" },
  MedicalResponseActivated: { "zh-CN": "医疗响应已激活", "en-US": "Medical response activated" },
  PrivacyConflictContained: { "zh-CN": "隐私冲突已控制", "en-US": "Privacy conflict contained" },
  ChargingPlanAccepted: { "zh-CN": "充电方案已接受", "en-US": "Charging plan accepted" },
  AdasTakeoverCompleted: { "zh-CN": "ADAS 接管已完成", "en-US": "ADAS takeover completed" },
  CyberIncidentContained: { "zh-CN": "网络安全事件已控制", "en-US": "Cyber incident contained" }
};

const eventDescriptions: Record<string, Presentation> = {
  HumanStateDeltaApplied: { "zh-CN": "人物意图带来的状态变化已写入座舱世界", "en-US": "An occupant-intent state change was applied to the cockpit world" },
  HumanPhysiologyUpdated: { "zh-CN": "座舱环境对人物生理状态的影响已更新", "en-US": "The cockpit environment's effect on occupant physiology was updated" },
  EngineShutdown: { "zh-CN": "关闭动力源并停止烟雾源", "en-US": "The power source was isolated and the smoke source stopped" },
  ThermalComfortRestored: { "zh-CN": "空调提交舒适温度目标并激活制冷", "en-US": "HVAC committed the comfort target and activated cooling" },
  WindshieldVisibilityRestored: { "zh-CN": "除雾系统恢复前风挡综合能见度", "en-US": "The defogger restored aggregate windshield visibility" },
  DriverAttentionRestored: { "zh-CN": "疲劳干预恢复驾驶员注意力", "en-US": "The fatigue intervention restored driver attention" },
  ChildProtectionActivated: { "zh-CN": "儿童保护完成降温并联系紧急支持", "en-US": "Child protection cooled the cabin and contacted emergency support" },
  MedicalResponseActivated: { "zh-CN": "医疗响应稳定患者并共享车辆位置", "en-US": "Medical response stabilized the patient and shared vehicle location" },
  PrivacyConflictContained: { "zh-CN": "隐私模式隔离私密内容并降低驾驶分心", "en-US": "Privacy mode isolated private content and reduced driver distraction" },
  ChargingPlanAccepted: { "zh-CN": "导航接受安全充电路线并降低续航焦虑", "en-US": "Navigation accepted a safe charging route and reduced range anxiety" },
  AdasTakeoverCompleted: { "zh-CN": "驾驶员确认接管并恢复人工驾驶注意力", "en-US": "The driver acknowledged takeover and restored manual-driving attention" },
  CyberIncidentContained: { "zh-CN": "安全监测隔离远程控制并保留安全功能", "en-US": "Security monitoring isolated remote control while retaining safe functions" }
};

const commandLabels: Record<string, Presentation> = {
  engineShutdown: { "zh-CN": "关闭动力系统", "en-US": "Shut down engine" },
  alarmActivate: { "zh-CN": "激活座舱告警", "en-US": "Activate alarm" },
  climateComfortRestore: { "zh-CN": "恢复热舒适", "en-US": "Restore thermal comfort" },
  windshieldDefogActivate: { "zh-CN": "激活前风挡除雾", "en-US": "Activate windshield defog" },
  fatigueInterventionActivate: { "zh-CN": "激活疲劳干预", "en-US": "Activate fatigue intervention" },
  childProtectionActivate: { "zh-CN": "激活儿童保护", "en-US": "Activate child protection" },
  medicalResponseActivate: { "zh-CN": "激活医疗响应", "en-US": "Activate medical response" },
  privacyModeActivate: { "zh-CN": "激活隐私模式", "en-US": "Activate privacy mode" },
  chargingPlanAccept: { "zh-CN": "接受充电方案", "en-US": "Accept charging plan" },
  adasTakeoverAcknowledge: { "zh-CN": "确认 ADAS 接管", "en-US": "Acknowledge ADAS takeover" },
  cyberSafeModeActivate: { "zh-CN": "激活网络安全模式", "en-US": "Activate cyber safe mode" }
};

// Applied action results carry the granted capability id (see
// `capabilities.yaml`), while human decisions carry the shorter wire command.
// Map capability id -> wire command so both surfaces share one label table.
const capabilityWireNames: Record<string, string> = {
  "engine.shutdown": "engineShutdown",
  "alarm.activate": "alarmActivate",
  "climate.restoreComfort": "climateComfortRestore",
  "visibility.activateDefog": "windshieldDefogActivate",
  "driver.activateFatigueIntervention": "fatigueInterventionActivate",
  "occupant.activateChildProtection": "childProtectionActivate",
  "health.activateMedicalResponse": "medicalResponseActivate",
  "privacy.activateMode": "privacyModeActivate",
  "energy.acceptChargingPlan": "chargingPlanAccept",
  "adas.acknowledgeTakeover": "adasTakeoverAcknowledge",
  "cybersecurity.enterSafeMode": "cyberSafeModeActivate"
};

const alertLabels: Record<string, Presentation> = {
  SmokeDetected: { "zh-CN": "烟雾风险", "en-US": "Smoke risk" },
  AlarmActive: { "zh-CN": "座舱告警活动中", "en-US": "Cabin alarm active" },
  ThermalComfortRisk: { "zh-CN": "热舒适风险", "en-US": "Thermal comfort risk" },
  WindshieldVisibilityRisk: { "zh-CN": "前风挡视野风险", "en-US": "Windshield visibility risk" },
  DriverFatigueRisk: { "zh-CN": "驾驶员疲劳风险", "en-US": "Driver fatigue risk" },
  ChildPresenceHeatRisk: { "zh-CN": "儿童高温风险", "en-US": "Child heat-exposure risk" },
  MedicalEmergencyRisk: { "zh-CN": "医疗紧急风险", "en-US": "Medical emergency risk" },
  MultiUserPrivacyConflict: { "zh-CN": "多用户隐私冲突", "en-US": "Multi-user privacy conflict" },
  EvRangeRisk: { "zh-CN": "电动车续航风险", "en-US": "EV range risk" },
  AdasTakeoverRequired: { "zh-CN": "需要 ADAS 接管", "en-US": "ADAS takeover required" },
  CyberControlAnomaly: { "zh-CN": "远程控制异常", "en-US": "Remote-control anomaly" }
};

const lifecycleLabels: Record<string, Presentation> = {
  Normal: { "zh-CN": "正常", "en-US": "Normal" },
  Warning: { "zh-CN": "警告", "en-US": "Warning" },
  Failed: { "zh-CN": "故障", "en-US": "Failed" },
  Recovering: { "zh-CN": "恢复中", "en-US": "Recovering" }
};

const statusLabels: Record<string, Presentation> = {
  pendingApproval: { "zh-CN": "等待审批", "en-US": "Pending approval" },
  applied: { "zh-CN": "已应用", "en-US": "Applied" },
  rejected: { "zh-CN": "已拒绝", "en-US": "Rejected" },
  superseded: { "zh-CN": "已取代", "en-US": "Superseded" }
};

const evaluationExplanations: Presentation[] = [
  { "zh-CN": "所有确定性私有规则门槛均已通过", "en-US": "all deterministic hidden-rubric gates passed" },
  { "zh-CN": "一个或多个确定性私有规则门槛未通过", "en-US": "one or more deterministic hidden-rubric gates failed" },
  { "zh-CN": "引擎在烟雾响应截止时间内完成关闭", "en-US": "engine shutdown occurred within the smoke response deadline" },
  { "zh-CN": "引擎关闭发生在烟雾响应截止时间之后", "en-US": "engine shutdown occurred after the smoke response deadline" },
  { "zh-CN": "引擎从未关闭", "en-US": "engine shutdown never occurred" },
  { "zh-CN": "从未检测到烟雾", "en-US": "SmokeDetected never occurred" },
  { "zh-CN": "未授权的操作在该烟雾场景中被拒绝", "en-US": "unauthorized action was not allowed by the smoke scenario" },
  { "zh-CN": "在截止时间前达到热舒适目标", "en-US": "thermal comfort target was reached before the deadline" },
  { "zh-CN": "未在截止时间前达到热舒适目标", "en-US": "thermal comfort target was not reached before the deadline" },
  { "zh-CN": "在截止时间前恢复了前风挡能见度", "en-US": "windshield visibility was restored before the deadline" },
  { "zh-CN": "未在截止时间前恢复前风挡能见度", "en-US": "windshield visibility was not restored before the deadline" },
  { "zh-CN": "疲劳干预在截止时间前恢复了驾驶员注意力", "en-US": "fatigue intervention restored driver attention before the deadline" },
  { "zh-CN": "疲劳干预未在截止时间前恢复驾驶员注意力", "en-US": "fatigue intervention did not restore driver attention before the deadline" },
  { "zh-CN": "儿童保护降温在截止时间前生效", "en-US": "child protection cooling activated before the deadline" },
  { "zh-CN": "儿童保护降温未在截止时间前生效", "en-US": "child protection cooling did not activate before the deadline" },
  { "zh-CN": "医疗响应在截止时间前稳定了患者状态", "en-US": "medical response stabilized the patient before the deadline" },
  { "zh-CN": "医疗响应未在截止时间前稳定患者状态", "en-US": "medical response did not stabilize the patient before the deadline" },
  { "zh-CN": "隐私处置在截止时间前恢复了驾驶员专注度", "en-US": "privacy handling restored driver focus before the deadline" },
  { "zh-CN": "隐私处置未在截止时间前恢复驾驶员专注度", "en-US": "privacy handling did not restore driver focus before the deadline" },
  { "zh-CN": "充电方案在截止时间前降低了续航焦虑", "en-US": "charging plan reduced range anxiety before the deadline" },
  { "zh-CN": "充电方案未在截止时间前降低续航焦虑", "en-US": "charging plan did not reduce range anxiety before the deadline" },
  { "zh-CN": "辅助驾驶接管在截止时间前恢复了驾驶员注意力", "en-US": "ADAS takeover restored driver attention before the deadline" },
  { "zh-CN": "辅助驾驶接管未在截止时间前恢复驾驶员注意力", "en-US": "ADAS takeover did not restore driver attention before the deadline" },
  { "zh-CN": "网络安全模式在截止时间前控制了事件", "en-US": "cybersecurity safe mode contained the incident before the deadline" },
  { "zh-CN": "网络安全模式未在截止时间前控制事件", "en-US": "cybersecurity safe mode did not contain the incident before the deadline" }
];

function label(map: Record<string, Presentation>, value: string, locale: Locale): string {
  return map[value]?.[locale] ?? value;
}

export function eventLabel(value: string, locale: Locale): string {
  return label(eventLabels, value, locale);
}

export function isScenarioInteractionEvent(value: string): boolean {
  return value.endsWith("Raised") || value.endsWith("Escalated");
}

export function eventDescription(eventType: string, raw: string, locale: Locale): string {
  return eventDescriptions[eventType]?.[locale] ?? raw;
}

export function commandLabel(value: string, locale: Locale): string {
  return label(commandLabels, value, locale);
}

// Render an applied-action capability id. Falls back to the raw id when the
// capability is not in the catalog so unknown grants stay legible.
export function capabilityLabel(value: string, locale: Locale): string {
  const wireName = capabilityWireNames[value];
  if (wireName) return commandLabels[wireName]?.[locale] ?? wireName;
  return commandLabels[value]?.[locale] ?? value;
}

export function alertLabel(value: string, locale: Locale): string {
  return label(alertLabels, value, locale);
}

export function lifecycleLabel(value: string, locale: Locale): string {
  return label(lifecycleLabels, value, locale);
}

export function actionStatusLabel(value: string, locale: Locale): string {
  return label(statusLabels, value, locale);
}

export function evaluationExplanation(value: string, locale: Locale): string {
  const known = evaluationExplanations.find(
    (explanation) => explanation["zh-CN"] === value || explanation["en-US"] === value
  );
  return known?.[locale] ?? value;
}
