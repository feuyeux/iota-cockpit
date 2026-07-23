# iota-cockpit 系统架构与设计图表集 (AI 生图提示词)

本文档包含 iota-cockpit 项目的完整架构图表生成提示词，涵盖系统架构、仿真执行流程、数字孪生物理模型、智能体系统、IPC 协议、录制评估和桌面应用七个维度。

---

## 目录

1. [系统架构总览](#1-系统架构总览)
   - 1.1 [分层架构与组件依赖](#11-分层架构与组件依赖)
   - 1.2 [完整运行时架构图](#12-完整运行时架构图)
   - 1.3 [架构总览海报](#13-架构总览海报)
2. [仿真执行流程](#2-仿真执行流程)
   - 2.1 [Tick 执行管线](#21-tick-执行管线)
   - 2.2 [Live Agent 工具循环](#22-live-agent-工具循环)
   - 2.3 [代码调用链路](#23-代码调用链路)
3. [数字孪生物理模型](#3-数字孪生物理模型)
   - 3.1 [双区车辆模型与热力学](#31-双区车辆模型与热力学)
4. [智能体系统与开放世界](#4-智能体系统与开放世界)
   - 4.1 [Agent 架构与 OpenWorld 运行时](#41-agent-架构与-openworld-运行时)
5. [IPC 协议与 Simulator 服务](#5-ipc-协议与-simulator-服务)
   - 5.1 [版本化 IPC 契约与会话认证](#51-版本化-ipc-契约与会话认证)
6. [录制与独立评估](#6-录制与独立评估)
   - 6.1 [录制存储与评估平面](#61-录制存储与评估平面)
7. [Desktop (Tauri) 架构](#7-desktop-tauri-架构)
   - 7.1 [Desktop 应用架构与通信流](#71-desktop-应用架构与通信流)
8. [场景与影响系统](#8-场景与影响系统)
   - 8.1 [场景加载、故障与影响规则](#81-场景加载故障与影响规则)

---

## 1. 系统架构总览

### 1.1 分层架构与组件依赖

**用途**: 展示 iota-cockpit 的四层架构和组件间的依赖关系

![分层架构与组件依赖](../img_result/layered_architecture.png)

**Prompt**:
```
Selected GPT-Image2 style-library template: Infographic Engine (ID: infographic-engine; category: Charts & Infographics; example cases: case 334, case 1, case 8).
Use case: infographic-diagram. Asset type: technical architecture diagram for project documentation. Output settings: high outputQuality, wide landscape 16:9 PNG, readable labels, clean layout.
Use a consistent technical infographic style for this entire wide landscape 16:9 image.
Use the shared visual system: clean technical infographic, warm off-white paper background, precise thin ink lines, subtle hand-drawn engineering paper texture, restrained cockpit amber accent, muted navy / forest green / terracotta / cyan / teal / gray module colors, readable labels, clear arrows, generous whitespace, no 3D, no neon, no stock cloud icons, no decorative blobs.
Palette: cockpit amber #D97706; deep navy #1E3A5F; forest green #2F6B4F; terracotta orange #C46A3A; protocol cyan #0E7490; backend teal #0F766E; neutral gray #52525B; paper background #F8F5EE.
Technical diagram rules: warm off-white paper, thin ink vector lines, rounded module rectangles, solid arrows, compact labels, clean sans-serif typography, disciplined spacing; keep labels short and readable; do not invent file paths, module names, database columns, commands, or backend names.
Negative details: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels.
Create a layered architecture diagram titled "iota-cockpit Component Layers and Dependencies".
The diagram is divided vertically into four distinct boxes representing layers with strict dependency flow:

1. Layer 1: "Presentation (Desktop & CLI)" - outlined in deep navy blue
   - Blocks: "apps/cockpit-desktop (React 19 + Vite 7 + Tailwind 4 + Lucide)", "Tauri 2 host (src-tauri/)"
   - Block: "cockpit-simulator CLI (validate, run, run-live, serve, bench, bench-live)"
   
2. Layer 2: "Orchestration & Agent Runtime" - outlined in muted forest green
   - Large block: "cockpit-agent (RuleAgent, HumanAgentDriver, OpenWorldRuntime, MultiAgentCoordinator, AcpAdapterConfig)"
   - Block: "cockpit-simulator (server, ipc/handler+lifecycle+control+open_world, live_run, benchmark)"
   - Block: "cockpit-plugin (PluginManifest, ProcessPluginExecutor, executable_sha256, StateDiff gating)"
   
3. Layer 3: "Simulation Core & Recording" - outlined in terracotta orange
   - Blocks: "cockpit-world (Simulation, WorldSnapshot, DigitalTwin, EffectKernel, CapabilityCatalog, InfluenceSchedule, Perception)"
   - Blocks: "cockpit-scenario (YAML loading, validation, hashing)"
   - Blocks: "cockpit-recording (RecordingStore, PayloadStore, RecordingQueue, diff, replay, replica)"
   - Blocks: "cockpit-evaluation (DeterministicEvaluator, DualJudgeEvaluator, EvaluationPolicy, HiddenRubric)"
   
4. Layer 4: "External Boundaries" - outlined in dark charcoal gray
   - Block: "iota-core (iota-sympantos-core)" (ACP backend, SkillRegistry, ephemeral sessions)
   - Block: "Backend AI Models" (Hermes, OpenCode via cockpit-judge providers)
   - Block: "SQLite Recording DB" (content-addressed SHA-256 payloads)
   - Block: "Scenario YAML" (scenarios/*.yaml, evaluations/private/*.yaml)

Connections and Flows:
- Amber solid arrows from "cockpit-agent" to "cockpit-world" labeled "tick step / action commit"
- Terracotta arrow from "cockpit-simulator" to "cockpit-agent" labeled "drives tool loop"
- Cyan arrow from "cockpit-agent" to "iota-core" labeled "ACP session (live-acp feature)"
- Green arrow from "cockpit-recording" to "SQLite Recording DB" labeled "persist tick payloads"
- Purple arrow from "cockpit-evaluation" to "cockpit-recording" labeled "reads immutable recording"
- Blue arrow from "Desktop" to "cockpit-simulator" labeled "TCP IPC (session-authenticated)"
- Gray arrow from "cockpit-world" to "Scenario YAML" labeled "initialization only"

Style instructions:
- Follow the technical diagram rules and palette stated at the top of this prompt
- Use compact, readable module labels and consistent font size within each layer
- Use continuous solid lines with no overlapping and generous white borders
- Do not invent module names, file paths, stores, backend names, or commands
```

---

### 1.2 完整运行时架构图

**用途**: 展示完整的运行时架构，包含所有模块、数据流和序列标记（双语版本）

![完整运行时架构图](../img_result/runtime_architecture.png)

**Prompt**:
```
Selected GPT-Image2 style-library template: Infographic Engine (ID: infographic-engine; category: Charts & Infographics; example cases: case 334, case 1, case 8).
Use case: infographic-diagram. Asset type: technical architecture diagram for project documentation. Output settings: high outputQuality, wide landscape 16:9 PNG, readable labels, clean layout.
Use a consistent technical infographic style for this entire wide landscape 16:9 image.
Use the shared visual system: clean technical infographic, warm off-white paper background, precise thin ink lines, subtle hand-drawn engineering paper texture, restrained cockpit amber accent, muted navy / forest green / terracotta / cyan / teal / gray module colors, readable labels, clear arrows, generous whitespace, no 3D, no neon, no stock cloud icons, no decorative blobs.
Palette: cockpit amber #D97706; deep navy #1E3A5F; forest green #2F6B4F; terracotta orange #C46A3A; protocol cyan #0E7490; backend teal #0F766E; neutral gray #52525B; paper background #F8F5EE.
Technical diagram rules: warm off-white paper, thin ink vector lines, rounded module rectangles, solid arrows, compact labels, clean sans-serif typography, disciplined spacing; keep labels short and readable; do not invent file paths, module names, database columns, commands, or backend names.
Negative details: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels.
Create a wide landscape technical architecture infographic titled "iota-cockpit Runtime Architecture / 运行时架构".
Use a precise module map, color-coded flow arrows, compact bilingual labels, and thin ink vector lines.

Canvas: wide landscape, 16:9 ratio, warm off-white paper background, rounded module columns, precise grid alignment.

Top legend:
- Amber: Simulator / Orchestration
- Navy: Desktop / Presentation
- Green: Agent Runtime
- Cyan: IPC Protocol
- Teal: World / Physics
- Purple: Recording / Evaluation
- Gray: Store / External

Sequence suffixes: D=Desktop, S=Simulator, A=Agent, W=World, R=Recording, E=Evaluation, I=IPC, P=Plugin

Main layout: 7 vertical columns + wide bottom store/external band

Column 1: Desktop / Presentation
Files: apps/cockpit-desktop/src/ (App.tsx, simulatorClient.ts, state/simulationReducer.ts, components/)
Show:
- React 19 + Vite 7 + TypeScript + Tailwind 4 + Lucide
- SimulationSourcePanel (scenario controls, run creation, live/rule mode)
- SimulationWorldView (entity state, cockpit systems, environment gauges)
- SimulationActivityFeed (realtime event stream, tool call traces)
- SimulationEvaluation (evaluation reports, judge agreement)
- SimulationNarrative (story-mode insights drawer)
- SimulationProgress (tick progress, run status)
- Keyboard shortcuts, i18n (en/zh), ErrorBoundary
- 1600x900 focus workspace layout

Column 2: Tauri Host & Sidecar Management
Files: apps/cockpit-desktop/src-tauri/ (src/, tauri.conf.json, prepare-sidecar.sh)
Show:
- Tauri 2 native host
- Sidecar discovery: COCKPIT_SIMULATOR_BIN or bundled cockpit-simulator-<target-triple>
- cockpit-evaluator sidecar for evaluation
- Private rubrics bundled as native resources (never exposed to webview)
- COCKPIT_JUDGE_A_BIN / COCKPIT_JUDGE_B_BIN configuration
- Recording DB persistence under application data directory

Column 3: Simulator Server & IPC
Files: crates/cockpit-simulator/src/ (main.rs, server.rs, ipc/handler.rs, ipc/lifecycle.rs, ipc/control.rs, ipc/open_world.rs, ipc/proto.rs, live_run.rs, benchmark.rs, memory.rs)
Show:
- CLI commands: validate, run, run-live, serve, bench, bench-live, mcp-bridge, policy-keygen/sign/revoke
- TCP serve on 127.0.0.1:0 (OS-assigned port, prints SIMULATOR_READY <addr>)
- Session token authentication (COCKPIT_SIMULATOR_SESSION_TOKEN preferred over CLI flag)
- Loopback-only bind guard (--allow-remote to override)
- IPC_VERSION = 7, tagged JSON protocol
- SimulatorHandler split: handler.rs (state/dispatch), lifecycle.rs (run create/start/step/resume/stop), control.rs (approval/plugins/replay/diff/recording), open_world.rs (entity/agent-goal)
- LiveTurnControl: CancellationToken for live ACP turn cancellation
- MAX_EVENT_HISTORY = 2048 event ring buffer
- Event cursor for reconnect recovery
- 1 MiB request cap
- Ping heartbeat liveness probe

Column 4: Agent Runtime
Files: crates/cockpit-agent/src/ (lib.rs, live/, open_world.rs, multi_agent.rs, native_mcp.rs, policy.rs, rule_policy_bundle.rs, acp_adapter.rs, acp_adapter/config.rs, acp_adapter/prompt.rs, iota_core_adapter.rs, skill.rs, translation.rs)
Show:
- RuleAgent: deterministic offline tool execution
- HumanAgentDriver: event-triggered backend/tool loops per human per tick
- OpenWorldRuntime: per-human versioned sessions, Goal/Plan/Skill/Tool lifecycle, episodic recall, budgets
- MultiAgentCoordinator: stable priority/agent ordering, duplicate target rejection
- IotaCoreAcpAdapter: persona/goal prompt, human-scoped tool schemas, redacted trace
- AcpAdapterConfig: backend, cwd, timeout_ms, native_mcp_bridge_command, native_mcp_state_path, native_mcp_transport
- COCKPIT_HERMES_BIN env override, ensure_cockpit_hermes_profile
- AcpAdapterError: InvalidBackend, Turn (fatal), Cancelled (clean stop)
- Native MCP bridge: stdio server for ACP session/new tool registration (NativeMcpTurnState with protocol_version, generation, expires_at_unix_ms)
- AgentRuntimePolicy: timeout-only execution, call-count/cost/wall-clock bounds
- RulePolicyBundle: Ed25519 signed manifests, policy revocation, audit log
- 10 typed simulation tools (get_observation, get_turn_context, list_visible_entities, inspect_sensor_quality, request_action, submit_decision, get_action_result, get_run_status, add_goal, wait_until)

Column 5: World Simulation Core
Files: crates/cockpit-world/src/ (simulation.rs, simulation/tick.rs, world.rs, digital_twin.rs, generated_vehicle_fire.rs, effects.rs, capability.rs, influence.rs, action.rs, sensor.rs, perception.rs, clock.rs, state_patch.rs, event.rs, plugin_failure.rs)
Show:
- Simulation: tick-based deterministic evolution, TICK_PHASE_ORDER (9 phases), TickPhaseHash per phase
- WorldSnapshot: 14-domain cockpit state (environment, climate, assistance, occupant, experience, mobility, connectivity, cybersecurity, devices, humans, alarm)
- DigitalTwin: coupled two-zone RC thermodynamics, water-vapour, barometric pressure, smoke/CO2/CO mass conservation, Beer-Lambert visibility, two-node occupant thermoregulation (humidity-limited evaporative loss), COHb exposure
- DigitalTwinParameters: runtime-owned physics profile (not from scenario YAML), CalibrationProvenance with source SHA-256
- EffectKernel: typed EffectPlan operations, IntentResolver, DeviceCapabilityResolver
- CapabilityCatalog: typed operations, entity/component gating
- InfluenceSchedule: versioned rules (CURRENT_INFLUENCE_RULE_VERSION=2), conflict arbitration (ConflictPolicy)
- ActionRequest/ActionResult: typed Action Gateway, capability enforcement
- Sensor/Observation: SensorQuality, perception delay, delivered_and_pending
- Fault injection at scheduled ticks
- PluginFailureRecord: per-tick plugin execution tracking

Column 6: Recording & Replay
Files: crates/cockpit-recording/src/ (lib.rs, store.rs, queue.rs, diff.rs, replay.rs, replica.rs)
Show:
- Recording: immutable, redacted, content-addressed SHA-256 payloads
- RecordingStore: SQLite with payload hashes and sizes only
- PayloadStore: content-addressed file storage with GC (PayloadGcReport)
- RecordingQueue: async sink with backpressure policy (RecordingQueuePolicy, RecordingQueueHealth)
- Schema version 2, runtime contract version 8, world-model version 8
- RecordingDiff: tick-by-tick comparison metrics (RecordingMetrics, TickDiff)
- replay_recording: deterministic replay verification
- AuthenticatedReplicaStore: payload restore evidence (PayloadRestoreEvidence)
- RecordedAuditEvent: redacted time-windowed projection for reconnect (WorldEvent, ToolCall, ActionResult, PluginFailure, HumanTurn, Error)
- OpenWorldCheckpoint: world + agent sessions for live restart recovery
- RecordingSeal / RecordingSigningKey: integrity verification

Column 7: Independent Evaluation
Files: crates/cockpit-evaluator/src/ (main.rs, suite.rs), crates/cockpit-evaluation/src/, crates/cockpit-judge/src/
Show:
- cockpit-evaluator: reads immutable Recording JSON or SQLite (read-only)
- Private rubric from evaluations/private/ (never passed to execution model)
- DeterministicEvaluator: evidence-based pass/fail/inconclusive
- DualJudgeEvaluator: two canonical-path-distinct judge providers
- cockpit-judge-hermes / cockpit-judge-opencode: ephemeral iota-core ACP sessions
- Judge identity/model/provenance validation, disagreement = inconclusive
- Suite mode: evaluations/suite.yaml, 10-scenario CI, baseline regression detection
- JSON + JUnit reports, exit code 2 on release gate failure
- SimulationEvaluationProgress telemetry

Bottom wide band: External Dependencies & Stores
Show:
- iota-core (iota-sympantos-core): ACP backend, SkillRegistry, ephemeral sessions, session/resume
- SQLite Recording DB: persistent tick storage, snapshot recovery
- Scenario YAML: scenarios/*.yaml (public initialization only)
- Private Rubrics: evaluations/private/*.yaml (evaluator-only)
- Backend AI Models: via cockpit-judge providers (Hermes, OpenCode)
- Native MCP state files: mode 0600 Unix, DACL Windows

Flow arrows:
- Navy arrows from Desktop to Tauri Host
- Amber arrows from Tauri/Simulator to Agent Runtime
- Green arrows from Agent to World (tool calls → state mutations)
- Teal arrows within World (tick phases, digital twin step)
- Cyan arrows for IPC protocol (Desktop ↔ Simulator)
- Purple arrows from Recording to Evaluation
- Gray arrows from all to Store/External band

Sequence markers (use circled markers):
1D Desktop connect, 2I IPC CreateLiveSimulationRun, 3S Simulator loads scenario, 4A Agent initializes OpenWorldRuntime, 5W World tick step (phases), 6A HumanAgentDriver tool loop, 7W Action Gateway commit, 8R Recording persist tick, 9I IPC event batch to Desktop, 10D WorldView/ActivityFeed render, 11E Evaluation on completed, 12E Dual Judge verdict

Visual style:
- Follow the technical diagram rules and palette stated at the top of this prompt
- Wide landscape infographic, 16:9 ratio
- Warm off-white paper background, thin rounded rectangles, precise grid alignment
- Bilingual labels: Chinese first, English second, separated by /
- Keep labels readable and concise
- Use small icons only when they clarify meaning: terminal, database, gear, shield, car, thermometer, gauge, network socket
- The image must look like an updated version of a reference architecture diagram, not a new unrelated poster

Negative prompt:
Unreadable tiny text, random fake file paths, obsolete modules, scoring in public scenarios, evaluator logic in simulator, semantic fallback, circuit breaker, wrong 6-phase tick order, excessive decorative art, messy arrows, 3D render, dark background, neon cyberpunk, stock cloud icons, blurry labels, Korean text, non-Chinese non-English labels
```

---

### 1.3 架构总览海报

**用途**: 以故事化的方式展示整体架构，适合用于文档封面或概览

![架构总览海报](../img_result/architecture_overview.png)

**Prompt**:
```
Selected GPT-Image2 style-library template: Poster Layout System (ID: poster-layout-system; category: Posters & Typography; example cases: case 345, case 5, case 10), with Infographic Engine constraints for structured technical labels.
Use case: productivity-visual and infographic-diagram. Asset type: technical story-board poster for project documentation. Output settings: high outputQuality, wide landscape 16:9 PNG, readable labels, clean layout.
Use a consistent technical story-board style for this entire wide landscape 16:9 image.
Use the shared visual system: clean technical infographic, warm off-white paper background, precise thin ink lines, subtle hand-drawn engineering paper texture, restrained cockpit amber accent, muted navy / forest green / terracotta / cyan / teal / gray module colors, readable labels, clear arrows, generous whitespace, no 3D, no neon, no stock cloud icons, no decorative blobs.
Palette: cockpit amber #D97706; deep navy #1E3A5F; forest green #2F6B4F; terracotta orange #C46A3A; protocol cyan #0E7490; backend teal #0F766E; neutral gray #52525B; paper background #F8F5EE.
Story poster rules: pen-and-ink technical story poster, warm paper texture, precise black linework, light cross-hatching, miniature engineering cutaway details, restrained cockpit amber highlights, readable labels, and a diagram-like composition.
Negative details: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels.
Use a pen-and-ink technical story poster layout with a hand-drawn architectural cutaway, light cross-hatching, precise black ink, warm paper texture, and restrained cockpit amber highlights.

Create a wide landscape story-board poster for the document "iota-cockpit architecture overview".

Scene: a transparent cutaway of a smart vehicle cockpit sits at the center like a precision instrument panel. Inside, miniature human occupants interact with labeled subsystems: climate control, ADAS, infotainment, cybersecurity shield, and occupant care. Around the vehicle, a deterministic simulation engine ticks like a clockwork mechanism, advancing physics through labeled phases. Small agent figures carry tool-call scrolls between the occupants and the simulation core. Below the vehicle, a recording tape captures every tick as immutable evidence. To the right, an independent evaluation tribunal with two judge figures examines the tape through rubric lenses. Above, a desktop monitor shows the world view and activity feed. The story should feel like a精密 but orderly miniature testing laboratory where every subsystem is observable and every action is traceable.

Composition: wide landscape poster, 16:9 ratio. Strong left-to-right hierarchy: Scenario input on the left, Simulation and Agent orchestration in the center, Recording and Evaluation on the right, Desktop observation above. Use arrows, pipes, labels, and little signs, but keep all text short and readable. Add the title "iota-cockpit Architecture" as hand-lettered text at the top.

Style: follow the story poster rules and palette stated at the top of this prompt. Keep the drawing precise, diagram-like, and legible; use subtle amber only on the active tick pulse and action gateway. No photorealism, no 3D render, no glossy UI mockup, no gradient background.

Mood: precise, investigative, a controlled laboratory where vehicle intelligence is tested through deterministic simulation and independent evidence.

Negative prompt: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels; also avoid blurry text, crowded random symbols, colored comic style, watercolor, oil paint, distorted terminals, broken arrows, and fake code blocks.
```

---

## 2. 仿真执行流程

### 2.1 Tick 执行管线

**用途**: 展示从场景加载到 tick 提交的完整执行管线，包括阶段顺序和故障注入

![Tick 执行管线](../img_result/tick_execution_pipeline.png)

**Prompt**:
```
Selected GPT-Image2 style-library template: Infographic Engine (ID: infographic-engine; category: Charts & Infographics; example cases: case 334, case 1, case 8).
Use case: infographic-diagram. Asset type: technical architecture diagram for project documentation. Output settings: high outputQuality, wide landscape 16:9 PNG, readable labels, clean layout.
Use a consistent technical infographic style for this entire wide landscape 16:9 image.
Use the shared visual system: clean technical infographic, warm off-white paper background, precise thin ink lines, subtle hand-drawn engineering paper texture, restrained cockpit amber accent, muted navy / forest green / terracotta / cyan / teal / gray module colors, readable labels, clear arrows, generous whitespace, no 3D, no neon, no stock cloud icons, no decorative blobs.
Palette: cockpit amber #D97706; deep navy #1E3A5F; forest green #2F6B4F; terracotta orange #C46A3A; protocol cyan #0E7490; backend teal #0F766E; neutral gray #52525B; paper background #F8F5EE.
Technical diagram rules: warm off-white paper, thin ink vector lines, rounded module rectangles, solid arrows, compact labels, clean sans-serif typography, disciplined spacing; keep labels short and readable; do not invent file paths, module names, database columns, commands, or backend names.
Negative details: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels.
Create a technical execution pipeline diagram titled "Simulation Tick Execution Pipeline".
The diagram shows a left-to-right flow with three major stages:

Stage 1: "Scenario Loading & Initialization" (outlined in navy blue)
- cockpit-scenario::load_scenario() parses YAML
- Validates schema_version, computes scenario_hash (SHA-256)
- Rejects evaluation, deadlineTick, rule IDs, thresholds in public YAML
- Builds SimulationScenario: entities, agents, public_goals, faults, influences, conflict_policy, clock, language
- Simulation::new() initializes WorldSnapshot with 14-domain state
- DigitalTwinParameters loaded (runtime-owned, not from scenario YAML)
- CapabilityCatalog registered for typed action validation

Stage 2: "Tick Step Execution" (outlined in forest green)
Shows TICK_PHASE_ORDER as a vertical sequence of 9 phases:
- Phase 1: "DigitalTwin" - advance_digital_twin() steps coupled two-zone RC thermodynamics, water-vapour balance, barometric pressure, smoke/CO2/CO mass conservation, Beer-Lambert visibility, two-node occupant thermoregulation (humidity-limited evaporative loss), COHb exposure
- Phase 2: "Faults" - Fault injection at scheduled at_tick (target, fault_type)
- Phase 3: "Influences" - InfluenceSchedule::schedule_due() collects due rules, arbitrate() resolves conflicts via ConflictPolicy, InfluenceDecision applied/deferred/rejected
- Phase 4: "PendingActions" - ActionRequest validated through CapabilityCatalog, EffectKernel resolves typed EffectPlan, capability enforcement per entity/component
- Phase 5: "HumanStateDeltas" - HumanStateDelta applied to occupant physiology and needs
- Phase 6: "StateDiffs" - Plugin and influence StateDiff patches applied with state_version gating
- Phase 7: "Perception" - enqueue_physical_event / enqueue_social_event, perception_delay_ticks, delivered_and_pending filtering
- Phase 8: "ActionResultEvents" - ActionResult emitted, ToolCallTrace captured, EventEnvelope entries assembled
- Phase 9: "Finalize" - WorldSnapshot advanced, state_version incremented, StepRecord emitted with TickPhaseHash per phase
- Each phase produces EventEnvelope entries and per-phase SHA-256 hash for determinism verification

Stage 3: "Recording & Output" (outlined in terracotta orange)
- StepRecord appended to Recording.ticks
- EventEnvelope entries captured (WorldEvent, ToolCall, ActionResult, PluginFailure)
- RecordingQueue async sink with backpressure policy
- RecordingStore persists to SQLite (content-addressed SHA-256 payloads)
- final_snapshot_hash computed for determinism verification
- IPC event batch emitted to connected Desktop clients

Connections:
- Blue arrows from Stage 1 to Stage 2 (initialized world enters tick loop)
- Green arrows through Stage 2 phases (sequential 9-phase execution)
- Orange arrows from Stage 2 to Stage 3 (committed state → recording)
- Red dashed arrow from "Faults" phase to "DigitalTwin" showing external risk injection affecting next tick

Style instructions:
- Follow the technical diagram rules and palette stated at the top of this prompt
- Use compact labels, clean sans-serif type, and consistent font size within each hierarchy level
- Use continuous solid lines with no overlapping or broken strokes
- Do not invent tick phases, fault types, or recording fields
```

---

### 2.2 Live Agent 工具循环

**用途**: 展示 Live Agent 模式下每个 human 的工具循环执行流程

![Live Agent 工具循环](../img_result/live_agent_tool_loop.png)

**Prompt**:
```
Selected GPT-Image2 style-library template: Infographic Engine (ID: infographic-engine; category: Charts & Infographics; example cases: case 334, case 1, case 8).
Use case: infographic-diagram. Asset type: technical architecture diagram for project documentation. Output settings: high outputQuality, wide landscape 16:9 PNG, readable labels, clean layout.
Use a consistent technical infographic style for this entire wide landscape 16:9 image.
Use the shared visual system: clean technical infographic, warm off-white paper background, precise thin ink lines, subtle hand-drawn engineering paper texture, restrained cockpit amber accent, muted navy / forest green / terracotta / cyan / teal / gray module colors, readable labels, clear arrows, generous whitespace, no 3D, no neon, no stock cloud icons, no decorative blobs.
Palette: cockpit amber #D97706; deep navy #1E3A5F; forest green #2F6B4F; terracotta orange #C46A3A; protocol cyan #0E7490; backend teal #0F766E; neutral gray #52525B; paper background #F8F5EE.
Technical diagram rules: warm off-white paper, thin ink vector lines, rounded module rectangles, solid arrows, compact labels, clean sans-serif typography, disciplined spacing; keep labels short and readable; do not invent file paths, module names, database columns, commands, or backend names.
Negative details: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels.
Create a technical flow diagram titled "Live Agent Tool Loop per Human per Tick".

The diagram shows the complete tool-loop lifecycle for one human agent during a single tick:

Top Section: "Scheduling & Wake" (outlined in navy blue)
- HumanAgentDriver checks event-driven schedule for each human
- Wake conditions: urgent perception event, explicit runtime work, IDLE_HUMAN_RECHECK_TICKS (3) expiry
- Routine sensor deltas batched until recheck cadence
- Social reaction cooldown (2 ticks) prevents utterance spam
- OpenWorldRuntime provides per-human session state (AgentLifecycle: Active/Waiting/Sleeping/Recovering)

Center Section: "Backend Tool Loop" (outlined in forest green)
Shows the iterative tool-call cycle:
1. "Build Context" block: HumanTurnContext assembled (perception events, world state, goals, episodic memory)
2. "Backend Call" block: IotaCoreAcpAdapter sends persona/goal prompt with human-scoped tool schemas to ACP backend
   - No eager complete observation; model chooses read tools on demand
   - Native MCP bridge registered in session/new (cockpit-simulator mcp-bridge stdio)
   - NativeMcpTurnState: protocol_version, generation, scenario, snapshot, tool_definitions, max_calls, max_cost, expires_at_unix_ms
   - AcpAdapterConfig: native_mcp_bridge_command, native_mcp_state_path, native_mcp_transport
3. "Tool Dispatch" block: model issues tool calls
   - Read tools: simulation.get_observation, simulation.get_turn_context, simulation.list_visible_entities, simulation.inspect_sensor_quality
   - Write tools: simulation.request_action (typed Action Gateway), simulation.submit_decision
   - Control tools: simulation.add_goal, simulation.wait_until (human-scoped only)
4. "Validation & Bounds" block (terracotta):
   - MAX_TOOL_CALLS_PER_TURN, MAX_TOOL_COST_PER_TURN (weighted)
   - MAX_HUMAN_TURN_WALL_TIME_MS = 120,000
   - MAX_TOOL_RESPONSE_BYTES = 1 MiB
   - Capability enforcement per entity/component
   - Action-result ownership boundaries
   - LiveTurnControl: CancellationToken for external cancel (CancelAgentTurn IPC)
5. "Decision Parse" block: parse_decision() extracts final disposition
   - HumanTurnDisposition: final (required)
   - Prose redacted from recordings (REDACTED_DECISION_TEXT)

Bottom Section: "Commit & Evidence" (outlined in purple)
- All tool exchanges captured in private per-turn transaction view
- Native MCP calls replayed through LocalMcpServer with same call IDs
- Bridge and parent results must agree; divergence aborts without committing
- Actions accepted only through simulation.request_action
- Backend failure or divergence → tick aborts (strict mode) or skips human (best-effort)
- HumanTurnEvidence emitted: tool calls, actions, decision kind, timing
- OpenWorldRuntime updated: episodic memory, conversation turns, relationship scores

Connections:
- Blue arrows from scheduling to backend loop
- Green arrows cycling through tool dispatch iterations
- Red arrows showing abort paths (failure, divergence, timeout)
- Purple arrows from commit to recording/evidence

Style instructions:
- Follow the technical diagram rules and palette stated at the top of this prompt
- Use compact labels, clean sans-serif type, and consistent font size
- Use continuous solid lines with no overlapping or broken strokes
- Do not invent tool names, bounds, or disposition types
```

---

### 2.3 代码调用链路

**用途**: 展示从 CLI/Desktop 入口到仿真提交边界的完整代码调用链

![代码调用链路](../img_result/code_call_chains.png)

**Prompt**:
```
Selected GPT-Image2 style-library template: Poster Layout System (ID: poster-layout-system; category: Posters & Typography; example cases: case 345, case 5, case 10), with Infographic Engine constraints for structured technical labels.
Use case: productivity-visual and infographic-diagram. Asset type: technical story-board poster for project documentation. Output settings: high outputQuality, wide landscape 16:9 PNG, readable labels, clean layout.
Use a consistent technical story-board style for this entire wide landscape 16:9 image.
Use the shared visual system: clean technical infographic, warm off-white paper background, precise thin ink lines, subtle hand-drawn engineering paper texture, restrained cockpit amber accent, muted navy / forest green / terracotta / cyan / teal / gray module colors, readable labels, clear arrows, generous whitespace, no 3D, no neon, no stock cloud icons, no decorative blobs.
Palette: cockpit amber #D97706; deep navy #1E3A5F; forest green #2F6B4F; terracotta orange #C46A3A; protocol cyan #0E7490; backend teal #0F766E; neutral gray #52525B; paper background #F8F5EE.
Story poster rules: pen-and-ink technical story poster, warm paper texture, precise black linework, light cross-hatching, miniature engineering cutaway details, restrained cockpit amber highlights, readable labels, and a diagram-like composition.
Negative details: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels.
Use a pen-and-ink sequential flow poster with hand-lettered annotations, light cross-hatching, mechanical process details, warm paper texture, and restrained cockpit amber highlights.

Create a wide landscape story-board poster for the document "iota-cockpit code call chains".

Scene: depict a journey of one simulation tick as a small precision gear traveling through a mechanical clockwork system. It begins at cockpit-simulator main.rs, passes command switches for validate, run, run-live, serve, bench, bench-live, and mcp-bridge, then splits into two illustrated paths: the offline RuleAgent route and the live ACP route.

RuleAgent route moves through:
- Scenario loading (cockpit-scenario::load_scenario)
- Simulation initialization (cockpit-world::Simulation::new)
- RuleAgent tool execution (cockpit-agent::RuleAgent)
- Tick step phases (digital_twin → faults → influences → pending_actions → human_state_deltas → state_diffs → perception → action_result_events → finalize)
- Recording capture (cockpit-recording::run_rule_agent_recording)
- Final output (Recording JSON)

Live ACP route shows:
- HumanAgentDriver scheduling per human
- IotaCoreAcpAdapter building persona prompt
- ACP session/new with native MCP bridge registration
- Tool loop: get_observation → request_action → submit_decision
- LocalMcpServer replay and result agreement check
- Action Gateway commit through EffectKernel
- Recording with redacted evidence

Desktop route shows:
- React App → simulatorClient.connect()
- TCP IPC to cockpit-simulator serve
- CreateLiveSimulationRun → StepLiveSimulation
- Event batch streaming back to ActivityFeed
- Evaluation on completed run

Composition: wide landscape poster, 16:9 ratio.
Arrange the call chain as a large board-game-like path with numbered stations and arrows.
Put "load → init → tick(digital_twin/faults/influences/pending_actions/human_state_deltas/state_diffs/perception/action_result_events/finalize) → record" as a clear ribbon across the middle.
Show external boundaries as illustrated gates: scenario YAML file, ACP child process, MCP stdio bridge, SQLite recording DB, and TCP socket.
Add the title "Code Call Chains" at the top and a small subtitle "from scenario to committed evidence".

Style: follow the story poster rules and palette stated at the top of this prompt. Keep crisp contour lines, readable miniature labels, and light amber accents only on the active tick gear and action gateway. Keep the journey metaphor technically accurate and structured.

Mood: precise engineering map, a tick crossing checkpoints and physics stages, clear enough to teach the simulation path at a glance.

Negative prompt: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels; also avoid unreadable spaghetti arrows, random pseudo-code, fantasy map cliches, excessive colors, cluttered icons, and distorted terminal text.
```

---

## 3. 数字孪生物理模型

### 3.1 双区车辆模型与热力学

**用途**: 展示数字孪生的耦合双区车辆物理模型，包括热力学、气体质量和乘员生理学

![双区车辆模型与热力学](../img_result/digital_twin_physics.png)

**Prompt**:
```
Selected GPT-Image2 style-library template: Infographic Engine (ID: infographic-engine; category: Charts & Infographics; example cases: case 334, case 1, case 8).
Use case: infographic-diagram. Asset type: technical architecture diagram for project documentation. Output settings: high outputQuality, wide landscape 16:9 PNG, readable labels, clean layout.
Use a consistent technical infographic style for this entire wide landscape 16:9 image.
Use the shared visual system: clean technical infographic, warm off-white paper background, precise thin ink lines, subtle hand-drawn engineering paper texture, restrained cockpit amber accent, muted navy / forest green / terracotta / cyan / teal / gray module colors, readable labels, clear arrows, generous whitespace, no 3D, no neon, no stock cloud icons, no decorative blobs.
Palette: cockpit amber #D97706; deep navy #1E3A5F; forest green #2F6B4F; terracotta orange #C46A3A; protocol cyan #0E7490; backend teal #0F766E; neutral gray #52525B; paper background #F8F5EE.
Technical diagram rules: warm off-white paper, thin ink vector lines, rounded module rectangles, solid arrows, compact labels, clean sans-serif typography, disciplined spacing; keep labels short and readable; do not invent file paths, module names, database columns, commands, or backend names.
Negative details: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels.
Create a technical physics model diagram titled "Digital Twin: Coupled Two-Zone Vehicle Model".

The diagram is divided into four interconnected sections:

Left Section: "Thermal Zones" (outlined in terracotta orange)
Shows the RC thermal network:
- Zone 1: "Cabin Air" - temperature, humidity (water-vapour balance)
- Zone 2: "Cabin Surfaces" - thermal mass, radiation coupling
- RC elements: thermal resistance (R), thermal capacitance (C)
- Heat sources: solar radiation, occupant metabolic heat, HVAC, external fire (measured HRR profile)
- Heat sinks: conduction to exterior, ventilation, smoke removal
- Calibrated from 1,302 closed-sedan observations (Mendeley DOI 10.17632/8mfgd8w9rg.1)
- 30% recursive holdout RMSE: 2.026942°C vs 2.916170°C persistence baseline

Center-Left Section: "Atmospheric & Gas Model" (outlined in cyan)
Shows mass conservation:
- Barometric pressure with leakage model
- Smoke density: mass conservation, smoke_removal_rate_s
- CO2 concentration: occupant generation, ventilation dilution
- CO concentration: combustion source (measured species yields), CFK-derived exposure
- Beer-Lambert visibility: extinction coefficient × path length → visibility distance
- NIST Fire Calorimetry profile (DOI 10.18434/mds2-2314): 6,468-row HRR trace
- 10-second lookup RMSE: 56.276435 kW vs 109.328921 kW persistence

Center-Right Section: "Occupant Physiology" (outlined in forest green)
Shows two-node thermoregulation:
- Core node: metabolic heat, blood flow regulation
- Skin node: evaporative heat loss (humidity-limited), convective/radiative exchange
- COHb exposure/recovery: CFK-derived AL=2 model
- Field-validated: 100 armored-vehicle crew members, peak RMSE 1.94%
- Gated for: 1-hour rest stability, passive seated hot-vs-moderate direction, 23-71% RH ordering
- PhysiologyDelta output per tick

Right Section: "Integration & Calibration" (outlined in purple)
Shows how the model integrates:
- DigitalTwinParameters: runtime-owned (not from scenario YAML)
- advance() called during tick "DigitalTwin" phase
- CabinZoneState output: temperature, humidity, pressure, smoke, CO2, CO, visibility
- PhysiologyState output: core_temp, skin_temp, cohb_pct, sweat_rate
- CalibrationProvenance: profile IDs, source SHA-256, model version
- Explicitly unfitted boundaries: exterior-fire-to-cabin transfer, pressure equalization, cohort generalization

Connections:
- Orange arrows showing heat flow between zones
- Cyan arrows showing gas mass transfer
- Green arrows showing physiology feedback to thermal model
- Purple arrows showing calibration data flow into parameters

Style instructions:
- Follow the technical diagram rules and palette stated at the top of this prompt
- Use compact labels, clean sans-serif type, and consistent font size
- Show physics relationships as labeled arrows with units
- Use continuous solid lines with no overlapping or broken strokes
- Do not invent calibration values, DOIs, or model parameters
```

---

## 4. 智能体系统与开放世界

### 4.1 Agent 架构与 OpenWorld 运行时

**用途**: 展示智能体系统的完整架构，包括 RuleAgent、LiveAgent 和 OpenWorld 运行时

![Agent 架构与 OpenWorld 运行时](../img_result/agent_open_world.png)

**Prompt**:
```
Selected GPT-Image2 style-library template: Infographic Engine (ID: infographic-engine; category: Charts & Infographics; example cases: case 334, case 1, case 8).
Use case: infographic-diagram. Asset type: technical architecture diagram for project documentation. Output settings: high outputQuality, wide landscape 16:9 PNG, readable labels, clean layout.
Use a consistent technical infographic style for this entire wide landscape 16:9 image.
Use the shared visual system: clean technical infographic, warm off-white paper background, precise thin ink lines, subtle hand-drawn engineering paper texture, restrained cockpit amber accent, muted navy / forest green / terracotta / cyan / teal / gray module colors, readable labels, clear arrows, generous whitespace, no 3D, no neon, no stock cloud icons, no decorative blobs.
Palette: cockpit amber #D97706; deep navy #1E3A5F; forest green #2F6B4F; terracotta orange #C46A3A; protocol cyan #0E7490; backend teal #0F766E; neutral gray #52525B; paper background #F8F5EE.
Technical diagram rules: warm off-white paper, thin ink vector lines, rounded module rectangles, solid arrows, compact labels, clean sans-serif typography, disciplined spacing; keep labels short and readable; do not invent file paths, module names, database columns, commands, or backend names.
Negative details: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels.
Create a technical agent architecture diagram titled "Agent System & OpenWorld Runtime".

The diagram is divided into four sections:

Left Section: "Agent Modes" (outlined in navy blue)
Shows the two execution modes:
1. RuleAgent (offline/deterministic):
   - LocalMcpServer handles tool calls directly
   - No backend AI model required
   - Used for: CLI run, Simulator protocol contracts, deterministic tests
   - rule_policy_bundle.rs: Ed25519 signed policy manifests
   - Policy selection, revocation, audit logging

2. LiveAgent (ACP backend):
   - IotaCoreAcpAdapter builds persona/goal prompt
   - AcpAdapterConfig: backend, cwd, timeout_ms, native_mcp_bridge_command, native_mcp_state_path, native_mcp_transport
   - COCKPIT_HERMES_BIN env override for Hermes executable
   - ensure_cockpit_hermes_profile: dedicated profile isolation
   - Human-scoped simulation tool schemas
   - No eager world observation
   - Completed tool exchanges fed into later rounds
   - Runtime events mapped to redacted trace
   - AcpAdapterError: InvalidBackend, Turn (fatal, no fallback), Cancelled (clean stop)
   - Execution-lock collision detection (iota-core CacheStore dedup)
   - Requires live-acp feature flag

Center-Left Section: "OpenWorldRuntime" (outlined in forest green)
Shows per-human session state:
- OPEN_WORLD_RUNTIME_VERSION = 3
- DEFAULT_CONCURRENT_AGENT_BUDGET = 8
- DEFAULT_AGENT_TOOL_BUDGET = 16
- Per-human versioned session with:
  - AgentLifecycle: Active, Waiting, Sleeping, Recovering, Completed, Failed
  - GoalState: Proposed, Active, Satisfied, Blocked, Abandoned (max 64 per agent)
  - PlanStep: Pending, Running, Waiting, Succeeded, Failed, Skipped
  - SkillState: skill_id, version, lifecycle, activated_tick
  - ToolState: tool_name, lifecycle, call_count
  - EpisodicMemory: bounded recall (max 256 per agent)
  - AcpConversationTurn: bounded history (max 64 per agent)
  - RelationshipState: evolving scores between agents
  - AgentBudget: weighted per-agent resource limits
- Deterministic priority scheduling
- Wait/wake/recovery/replan transitions
- Retired-entity tracking
- Fresh iota-core logical sessions prevent cross-human inheritance
- Session resume: ACP session/resume or capability-gated session/load

Center-Right Section: "HumanAgentDriver" (outlined in terracotta orange)
Shows the per-tick orchestration:
- Event-triggered scheduling (not every human every tick)
- IDLE_HUMAN_RECHECK_TICKS = 3
- SOCIAL_REACTION_COOLDOWN_TICKS = 2
- MAX_HUMAN_TURN_WALL_TIME_MS = 120,000
- Transient utterances (raw for live, redacted for recording)
- Tool sequence counter for deterministic ordering
- LiveTickMode: Strict (all must succeed) or BestEffort (skip failures)
- HumanTurnEvidence: tool calls, actions, decision, timing

Right Section: "Multi-Agent & Policy" (outlined in purple)
Shows coordination:
- MultiAgentCoordinator:
  - Stable priority/agent ordering
  - Rejects duplicate target commands deterministically
  - AgentActionBatch processing
- AgentRuntimePolicy:
  - Timeout-only execution
  - Call-count bounds
  - Weighted tool-cost limits
  - Wall-clock limits
  - Response-size limits
  - Pagination bounds
  - Capability enforcement
  - Action-result ownership
- Translation: IdentityTranslator, normalize_language, same_language

Connections:
- Blue arrows from agent modes to OpenWorldRuntime
- Green arrows from OpenWorldRuntime to HumanAgentDriver
- Orange arrows from HumanAgentDriver to World (tool calls)
- Purple arrows from Multi-Agent coordinator to individual agents

Style instructions:
- Follow the technical diagram rules and palette stated at the top of this prompt
- Use compact labels, clean sans-serif type, and consistent font size
- Use continuous solid lines with no overlapping or broken strokes
- Do not invent lifecycle states, bounds, or tool names
```

---

## 5. IPC 协议与 Simulator 服务

### 5.1 版本化 IPC 契约与会话认证

**用途**: 展示 Simulator IPC 服务器的协议设计、认证机制和命令集

![版本化 IPC 契约与会话认证](../img_result/ipc_protocol.png)

**Prompt**:
```
Selected GPT-Image2 style-library template: Infographic Engine (ID: infographic-engine; category: Charts & Infographics; example cases: case 334, case 1, case 8).
Use case: infographic-diagram. Asset type: technical architecture diagram for project documentation. Output settings: high outputQuality, wide landscape 16:9 PNG, readable labels, clean layout.
Use a consistent technical infographic style for this entire wide landscape 16:9 image.
Use the shared visual system: clean technical infographic, warm off-white paper background, precise thin ink lines, subtle hand-drawn engineering paper texture, restrained cockpit amber accent, muted navy / forest green / terracotta / cyan / teal / gray module colors, readable labels, clear arrows, generous whitespace, no 3D, no neon, no stock cloud icons, no decorative blobs.
Palette: cockpit amber #D97706; deep navy #1E3A5F; forest green #2F6B4F; terracotta orange #C46A3A; protocol cyan #0E7490; backend teal #0F766E; neutral gray #52525B; paper background #F8F5EE.
Technical diagram rules: warm off-white paper, thin ink vector lines, rounded module rectangles, solid arrows, compact labels, clean sans-serif typography, disciplined spacing; keep labels short and readable; do not invent file paths, module names, database columns, commands, or backend names.
Negative details: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels.
Create a technical protocol diagram titled "Simulator IPC Contract & Session Authentication".

The diagram is divided into three sections:

Left Section: "Connection & Authentication" (outlined in navy blue)
Shows the connection lifecycle:
- Server binds 127.0.0.1:0 (OS-assigned port)
- Prints "SIMULATOR_READY <addr>" to stdout
- Session token: COCKPIT_SIMULATOR_SESSION_TOKEN env (preferred) or --session-token flag
- Token compared in plaintext over loopback (no TLS)
- Loopback-only guard: rejects non-loopback without --allow-remote
- MAX_IPC_REQUEST_BYTES = 1 MiB
- Newline-delimited JSON framing
- IPC_VERSION = 7 (tagged protocol)
- Event cursor for reconnect recovery

Center Section: "Command Set" (outlined in forest green)
Shows SimulatorCommand variants:
- Lifecycle: ValidateScenario, CreateSimulationRun, CreateLiveSimulationRun, ResumeSimulation, ResumeLiveSimulation
- Control: StartSimulation, PauseSimulation, StepSimulation, StepLiveSimulation, StopSimulation, CancelLiveTurn, CancelAgentTurn, SetApprovalRequired
- Entity: SpawnEntity, RemoveEntity
- Agent: AddAgentGoal, SetAgentGoalStatus, WaitAgentUntil, GetOpenWorldRuntime, CheckpointOpenWorld
- Approval: ApproveAction, RejectAction
- Policy: SelectRulePolicy, ListRulePolicies
- Query: GetSimulationSnapshot, GetSimulationRunStatus, GetSimulationEvents (cursor), GetRecordedAuditEvents (durable pagination with after_sequence/tail_limit), GetAgentTrace
- Inspection: StartReplay, DiffRecordings
- Liveness: Ping (seq-based heartbeat probe)

Right Section: "Response & Events" (outlined in terracotta orange)
Shows SimulatorResponse and streaming:
- Tagged JSON responses per command (version, correlation_id, ok, result, error)
- IpcError with code, message, details, run_id, tick, correlationId
- SimulatorEvent tagged union (all carry cursor):
  - SimulationStateChanged (state, run_id)
  - SimulationTickCommitted (tick, sim_time_ms, version)
  - SimulationEvent (EventEnvelope)
  - SimulationToolCall (ToolCallTrace)
  - SimulationHumanTurn (tick, backend, HumanTurnEvidence)
  - SimulationActionResult (ActionResult)
  - SimulationPluginFailure (PluginFailureRecord)
  - SimulationEvaluationProgress (EvaluationProgress: run_id, recorded_ticks, status)
  - SimulationError (IpcError)
- RunStatus: idle, running, paused, completed, stopped, failed
- Persistent mode: RecordingStore saves committed ticks
- ResumeSimulation restores snapshot and event cursor
- GetRecordedAuditEvents: durable redacted evidence for reconnect after cursor expiry

Connections:
- Blue arrows showing connection establishment
- Green arrows for command dispatch
- Orange arrows for response/event streaming
- Red arrows showing error paths

Style instructions:
- Follow the technical diagram rules and palette stated at the top of this prompt
- Use compact labels, clean sans-serif type, and consistent font size
- Show protocol messages in monospace font
- Use continuous solid lines with no overlapping or broken strokes
- Do not invent command names, error codes, or protocol versions
```

---

## 6. 录制与独立评估

### 6.1 录制存储与评估平面

**用途**: 展示录制系统的不可变存储设计和独立评估平面的双判官机制

![录制存储与评估平面](../img_result/recording_evaluation.png)

**Prompt**:
```
Selected GPT-Image2 style-library template: Infographic Engine (ID: infographic-engine; category: Charts & Infographics; example cases: case 334, case 1, case 8).
Use case: infographic-diagram. Asset type: technical architecture diagram for project documentation. Output settings: high outputQuality, wide landscape 16:9 PNG, readable labels, clean layout.
Use a consistent technical infographic style for this entire wide landscape 16:9 image.
Use the shared visual system: clean technical infographic, warm off-white paper background, precise thin ink lines, subtle hand-drawn engineering paper texture, restrained cockpit amber accent, muted navy / forest green / terracotta / cyan / teal / gray module colors, readable labels, clear arrows, generous whitespace, no 3D, no neon, no stock cloud icons, no decorative blobs.
Palette: cockpit amber #D97706; deep navy #1E3A5F; forest green #2F6B4F; terracotta orange #C46A3A; protocol cyan #0E7490; backend teal #0F766E; neutral gray #52525B; paper background #F8F5EE.
Technical diagram rules: warm off-white paper, thin ink vector lines, rounded module rectangles, solid arrows, compact labels, clean sans-serif typography, disciplined spacing; keep labels short and readable; do not invent file paths, module names, database columns, commands, or backend names.
Negative details: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels.
Create a technical data pipeline diagram titled "Recording Store & Independent Evaluation Plane".

The diagram is divided into three sections:

Left Section: "Recording Architecture" (outlined in navy blue)
Shows the immutable recording design:
- Recording struct: run_id, scenario_hash, seed, ticks[], final_snapshot_hash
- Content-addressed SHA-256 payloads (PayloadStore)
- SQLite stores only payload hashes and sizes (RecordingStore)
- Recording headers: runtime/world-model versions, commit, plugin hashes, scenario hash, seed, clock config
- Schema version 2, runtime contract version 8, world-model version 8
- serialize_redacted_recording: free-form prose redacted
- RecordingQueue: async sink with backpressure (RecordingQueuePolicy)
- RecordingDiff: tick-by-tick comparison (RecordingMetrics, TickDiff)
- replay_recording: deterministic replay verification
- OpenWorldCheckpoint: world + agent sessions for restart recovery

Center Section: "Evaluation Plane" (outlined in forest green)
Shows the independent evaluation architecture:
- cockpit-evaluator: separate process, reads immutable Recording
- Input: Recording JSON or SQLite (RecordingStore::open_read_only)
- Private rubric: evaluations/private/*.yaml (never passed to execution model)
- DeterministicEvaluator:
  - Evidence-based scoring (tick/entity/event citations)
  - Verdict: pass / fail / inconclusive
  - Input, rubric, prompt, model, schema hashes attached
- DualJudgeEvaluator:
  - Two canonical-path-distinct judge providers
  - --judge-a-command / --judge-b-command (must differ)
  - --judge-a-arg / --judge-b-arg (identity, model, workspace, transport)
  - Per-provider timeout and output limits
  - Duplicate identity/model → inconclusive
  - Any disagreement → inconclusive, fails release gate

Right Section: "Judge Providers & Suite" (outlined in terracotta orange)
Shows the concrete judge implementations:
- cockpit-judge-hermes:
  - Invokes real model via ephemeral iota-core ACP session
  - No credentials on argv/stdin
  - Strictly parses bare model JSON decision
  - Trusted provenance attached outside model
- cockpit-judge-opencode:
  - Same contract, different executable path
  - Different model required
- Suite mode (evaluations/suite.yaml):
  - 10-scenario CI suite
  - --simulator-command launches separate Simulator process
  - --baseline detects pass-to-non-pass regressions
  - --minimum-pass-rate (default 1.0)
  - JSON + JUnit reports
  - Exit code 2 on release gate failure
- Desktop integration:
  - COCKPIT_JUDGE_A_BIN / COCKPIT_JUDGE_B_BIN
  - COCKPIT_JUDGE_A_ARGS_JSON / COCKPIT_JUDGE_B_ARGS_JSON
  - Automatic evaluation on completed run
  - Manual retry, judge agreement, export, history

Connections:
- Blue arrows from Recording to Evaluation (immutable input)
- Green arrows from Evaluation to Judge providers
- Orange arrows from Suite to Simulator (scenario execution)
- Purple arrows showing verdict flow back to reports
- Red arrows showing release gate failure paths

Style instructions:
- Follow the technical diagram rules and palette stated at the top of this prompt
- Use compact labels, clean sans-serif type, and consistent font size
- Use continuous solid lines with no overlapping or broken strokes
- Do not invent evaluation fields, judge commands, or verdict types
```

---

## 7. Desktop (Tauri) 架构

### 7.1 Desktop 应用架构与通信流

**用途**: 展示 cockpit-desktop Tauri 应用的前后端架构、Simulator 通信和状态管理

![Desktop 应用架构与通信流](../img_result/desktop_tauri_architecture.png)

**Prompt**:
```
Selected GPT-Image2 style-library template: Infographic Engine (ID: infographic-engine; category: Charts & Infographics; example cases: case 334, case 1, case 8).
Use case: infographic-diagram. Asset type: technical architecture diagram for project documentation. Output settings: high outputQuality, wide landscape 16:9 PNG, readable labels, clean layout.
Use a consistent technical infographic style for this entire wide landscape 16:9 image.
Use the shared visual system: clean technical infographic, warm off-white paper background, precise thin ink lines, subtle hand-drawn engineering paper texture, restrained cockpit amber accent, muted navy / forest green / terracotta / cyan / teal / gray module colors, readable labels, clear arrows, generous whitespace, no 3D, no neon, no stock cloud icons, no decorative blobs.
Palette: cockpit amber #D97706; deep navy #1E3A5F; forest green #2F6B4F; terracotta orange #C46A3A; protocol cyan #0E7490; backend teal #0F766E; neutral gray #52525B; paper background #F8F5EE.
Technical diagram rules: warm off-white paper, thin ink vector lines, rounded module rectangles, solid arrows, compact labels, clean sans-serif typography, disciplined spacing; keep labels short and readable; do not invent file paths, module names, database columns, commands, or backend names.
Negative details: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels.
Create a technical application architecture diagram titled "cockpit-desktop (Tauri 2) Architecture & Communication Flow".

The diagram is divided into four vertical sections:

Left Section: "React Frontend" (outlined in cyan)
Shows the React UI architecture:
- Components:
  - App.tsx (root, keyboard shortcuts, i18n, ErrorBoundary)
  - SimulationSourcePanel.tsx (scenario selection, run creation, live/rule mode, auto-run)
  - SimulationWorldView.tsx (entity state, cockpit systems, environment gauges, 14-domain view)
  - SimulationActivityFeed.tsx (realtime event stream, tool call traces, human turns)
  - SimulationEvaluation.tsx (evaluation reports, judge agreement, export)
  - SimulationNarrative.tsx (story-mode insights drawer)
  - SimulationProgress.tsx (tick progress, run status bar)
  - IndependentEvaluationPanel.tsx (dual judge results, provenance)
  - KeyboardShortcutsHelp.tsx (shortcut overlay)

- State Management:
  - state/simulationReducer.ts (Redux-style reducer)
  - SimulationModel states: connectedIdle, disconnected, scenarioLoading, runCreating, running, paused, ready, completed, stopped, failed
  - simulatorClient.ts (TCP IPC client with reconnect)
  - utils/reconnect.ts (exponentialBackoff)
  - utils/storage.ts (persisted session)

- UI Features:
  - 1600x900 focus workspace (scenario + world + activity visible together)
  - Evaluation and narrative in insights drawer (on demand)
  - i18n (en/zh)
  - Keyboard shortcuts (? for help, Escape to close)

Center-Left Section: "Simulator Client" (outlined in purple)
Shows the IPC communication layer:
- simulatorClient:
  - connect(): TCP to simulator sidecar
  - snapshot(cursor): batch event fetch with cursor
  - createRun / createLiveRun / resumeRun / resumeLiveRun
  - step / stepLive / start / pause / stop / cancelLiveTurn / cancelAgentTurn
  - approveAction / rejectAction / setApprovalRequired
  - spawnEntity / removeEntity
  - addGoal / setGoalStatus / waitUntil
  - getOpenWorldRuntime / checkpointOpenWorld
  - selectRulePolicy / listRulePolicies
  - getRecordedAuditEvents (durable reconnect pagination)
  - startReplay / diffRecordings
  - ping (heartbeat liveness probe)
  - evaluate / getEvaluationHistory

- Protocol:
  - TCP connection to simulator (OS-assigned port from SIMULATOR_READY)
  - Session token authentication
  - Newline-delimited JSON (IPC_VERSION = 7)
  - Event cursor for reconnect recovery
  - resetRequired on cursor gap

Center-Right Section: "Tauri Host" (outlined in orange)
Shows the native host layer:
- Sidecar management:
  - COCKPIT_SIMULATOR_BIN or bundled cockpit-simulator-<target-triple>
  - prepare-sidecar.sh builds release binaries
  - cockpit-evaluator sidecar for evaluation
- Private rubrics as native resources (never exposed to webview)
- Judge configuration:
  - COCKPIT_JUDGE_A_BIN / COCKPIT_JUDGE_B_BIN
  - COCKPIT_JUDGE_A_ARGS_JSON / COCKPIT_JUDGE_B_ARGS_JSON
  - COCKPIT_JUDGE_TIMEOUT_MS
- Recording DB under application data directory
- Evaluation report persistence
- Icon set from cockpit-icon.svg (PNG/ICNS/ICO)

Right Section: "Simulator Sidecar" (outlined in forest green)
Shows the backend process:
- cockpit-simulator serve:
  - Persistent mode with --recording-db
  - Session token from COCKPIT_SIMULATOR_SESSION_TOKEN
  - Prints SIMULATOR_READY <addr>
  - Optional --rule-policy-bundle with Ed25519 verification
- SimulatorHandler:
  - Manages Simulation lifecycle
  - HumanAgentDriver for live ticks
  - RecordingStore persistence
  - Event cursor tracking
  - OpenWorldRuntime checkpoint/restore
- Process restart recovery:
  - Restores snapshot from recording DB
  - Restores event cursor state
  - Verified by process_restart_recovery.rs test

Connections and Flow:
- Cyan arrows from React components to simulatorClient (invoke)
- Purple arrows from simulatorClient to Tauri host (sidecar management)
- Orange arrows from Tauri host to Simulator sidecar (process lifecycle)
- Green arrows from Simulator to React (event streaming via IPC)
- Red arrows showing error/disconnect paths

Sequence markers (use circled markers):
1. User selects scenario in SimulationSourcePanel
2. simulatorClient.createLiveRun() → IPC CreateLiveSimulationRun
3. Simulator loads scenario, initializes Simulation
4. simulatorClient.stepLive() → IPC StepLiveSimulation
5. HumanAgentDriver runs tool loop per human
6. World tick commits, events emitted
7. IPC event batch → simulatorClient
8. simulationReducer processes events
9. SimulationWorldView + ActivityFeed re-render
10. On completed: automatic evaluation triggered
11. cockpit-evaluator sidecar reads recording
12. Evaluation report displayed in IndependentEvaluationPanel

Style instructions:
- Follow the technical diagram rules and palette stated at the top of this prompt
- Use compact labels, clean sans-serif type, and consistent font size within each hierarchy level
- Show TypeScript/Rust type signatures in monospace font
- Use continuous solid lines with no overlapping or broken strokes
- Do not invent Tauri commands, IPC messages, or frontend components
```

---

## 8. 场景与影响系统

### 8.1 场景加载、故障与影响规则

**用途**: 展示场景 YAML 的严格初始化设计、故障注入和版本化影响规则系统

![场景加载、故障与影响规则](../img_result/scenario_influence_system.png)

**Prompt**:
```
Selected GPT-Image2 style-library template: Infographic Engine (ID: infographic-engine; category: Charts & Infographics; example cases: case 334, case 1, case 8).
Use case: infographic-diagram. Asset type: technical architecture diagram for project documentation. Output settings: high outputQuality, wide landscape 16:9 PNG, readable labels, clean layout.
Use a consistent technical infographic style for this entire wide landscape 16:9 image.
Use the shared visual system: clean technical infographic, warm off-white paper background, precise thin ink lines, subtle hand-drawn engineering paper texture, restrained cockpit amber accent, muted navy / forest green / terracotta / cyan / teal / gray module colors, readable labels, clear arrows, generous whitespace, no 3D, no neon, no stock cloud icons, no decorative blobs.
Palette: cockpit amber #D97706; deep navy #1E3A5F; forest green #2F6B4F; terracotta orange #C46A3A; protocol cyan #0E7490; backend teal #0F766E; neutral gray #52525B; paper background #F8F5EE.
Technical diagram rules: warm off-white paper, thin ink vector lines, rounded module rectangles, solid arrows, compact labels, clean sans-serif typography, disciplined spacing; keep labels short and readable; do not invent file paths, module names, database columns, commands, or backend names.
Negative details: unreadable tiny text, fake modules, fake code, wrong file paths, obsolete modules, crowded arrows, messy layout, 3D render, neon cyberpunk, glossy corporate dashboard, stock cloud architecture icons, gradient blobs, random decorative symbols, Korean text, non-Chinese non-English labels.
Create a technical scenario system diagram titled "Scenario Loading, Faults & Influence Rules".

The diagram is divided into three sections:

Left Section: "Scenario YAML Structure" (outlined in navy blue)
Shows the strict initialization resource design:
- Public scenarios (scenarios/*.yaml):
  - id, schema_version, scenario_hash (SHA-256)
  - seed (deterministic RNG)
  - clock: ClockConfig (tick_duration_ms, mode)
  - language: "en" or "zh" (BCP-47-ish)
  - outer_environment: OuterEnvironmentState
  - environment: CabinEnvironment (initial thermal/gas state)
  - humans: Vec<HumanState> (persona, BigFiveTraits, NeedsState)
  - devices: Vec<DeviceState> (DeviceLifecycle)
  - alarm: AlarmState
  - faults: Vec<Fault> (at_tick, target, fault_type)
  - agent: AgentGrant (primary)
  - agents: Vec<AgentGrant> (multi-agent)
  - public_goals: Vec<String> (non-scoring, visible to operators)
  - max_ticks: runtime horizon only
  - influences: Vec<InfluenceRule> (scheduled external risks)
  - conflict_policy: ConflictPolicy

- REJECTED in public YAML (parser enforcement):
  - evaluation, deadlineTick, rule IDs, thresholds, action mappings, release gates
  - These live exclusively under evaluations/private/

- 10 benchmark scenarios (14-domain taxonomy):
  - smoke-in-cockpit, heatwave-thermal-comfort, winter-defog-visibility
  - driver-fatigue-guardian, child-left-behind, medical-emergency
  - voice-privacy-conflict, ev-range-anxiety
  - adas-takeover-construction, cybersecurity-anomalous-control

Center Section: "Fault Injection" (outlined in terracotta orange)
Shows the fault system:
- Fault struct: at_tick, target, fault_type
- Applied during tick "Fault" phase
- Targets: entity IDs, component paths
- Types: sensor degradation, device failure, environmental disturbance
- Deterministic: same seed + same faults = same outcome
- Faults drive reproducible external risks, NOT successful domain interventions
- Simulation core remains sole authority for validation and state commits

Right Section: "Influence Rule System" (outlined in forest green)
Shows the versioned influence architecture:
- InfluenceRule:
  - CURRENT_INFLUENCE_RULE_VERSION
  - Scheduled via InfluenceSchedule
  - InfluenceOp: typed state mutation operation
  - InfluencePatch: target component path + value
  - Subscription: event-driven triggering
- Arbitration:
  - schedule_due() collects rules due at current tick
  - arbitrate() resolves conflicts when multiple rules target same component
  - ConflictPolicy: deterministic resolution strategy
  - InfluenceDecision: applied, deferred, rejected
  - ArbitrationOutcome: final resolution record
- StateDiff application:
  - StatePatchTarget: entity + component path
  - StateDiff: typed value change
  - State version advancement on commit
- Plugin gating (cockpit-plugin):
  - PluginManifest: id, version, api_contract, permissions, schema, hash, signature, command, filesystem_read_paths, executable_sha256
  - Manifest hash/API/permissions validation
  - executable_sha256 verified against on-disk binary before every spawn
  - ProcessPluginExecutor: out-of-process execution with permission gating and tick_budget_ms deadline
  - PluginPermission: WorldRead, WorldWrite, Network, FilesystemRead, ChildProcess, Threads
  - PluginFailurePolicy: DisablePlugin, PauseRun, FailRun
  - StateDiff gated by entity, component path, state version, value range
  - macOS process sandbox (sandbox-exec) with filesystem_read_paths allowlist

Connections:
- Blue arrows from YAML to Simulation initialization
- Orange arrows from faults to tick Fault phase
- Green arrows from influence rules to tick Influence phase
- Purple arrows from plugin validation to StateDiff application
- Red arrows showing rejected fields (parser enforcement)

Style instructions:
- Follow the technical diagram rules and palette stated at the top of this prompt
- Use compact labels, clean sans-serif type, and consistent font size
- Show YAML structure in monospace font
- Use continuous solid lines with no overlapping or broken strokes
- Do not invent scenario fields, fault types, or influence operations
```

---

## 附录：图表使用指南

### 图表索引

| 编号 | 标题 | 类型 | 用途 |
| :---| :---| :---| :---|
| 1.1 | 分层架构与组件依赖 | 技术图 | 展示四层架构和模块依赖关系 |
| 1.2 | 完整运行时架构图 | 技术图 | 展示所有模块、数据流和序列标记 |
| 1.3 | 架构总览海报 | 海报 | 故事化展示整体架构 |
| 2.1 | Tick 执行管线 | 技术图 | 展示从场景到提交的执行管线 |
| 2.2 | Live Agent 工具循环 | 技术图 | 展示每 human 每 tick 的工具循环 |
| 2.3 | 代码调用链路 | 海报 | 故事化展示调用链路 |
| 3.1 | 双区车辆模型与热力学 | 技术图 | 展示数字孪生物理模型 |
| 4.1 | Agent 架构与 OpenWorld 运行时 | 技术图 | 展示智能体系统完整架构 |
| 5.1 | 版本化 IPC 契约与会话认证 | 技术图 | 展示 IPC 协议和认证机制 |
| 6.1 | 录制存储与评估平面 | 技术图 | 展示录制和独立评估架构 |
| 7.1 | Desktop 应用架构与通信流 | 技术图 | 展示 Tauri 应用架构 |
| 8.1 | 场景加载、故障与影响规则 | 技术图 | 展示场景系统和影响规则 |

### 使用场景

| 场景 | 推荐图表 |
| :---| :---|
| 新人入职，了解整体架构 | 1.3, 1.2, 1.1 |
| 理解仿真执行流程 | 2.1, 2.2, 2.3 |
| 开发物理模型 | 3.1 |
| 开发智能体功能 | 4.1, 2.2 |
| 开发 IPC/通信 | 5.1, 7.1 |
| 开发录制/评估 | 6.1 |
| 开发 Desktop 应用 | 7.1 |
| 编写场景 | 8.1 |
| 技术分享 | 1.3, 2.3, 3.1 |
| 文档封面 | 1.3 |
