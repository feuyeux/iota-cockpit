# Cockpit Simulator 分层架构与请求链路流程

## 目录

1. [系统架构概览](#系统架构概览)
2. [分层架构详解](#分层架构详解)
3. [请求链路流程](#请求链路流程)
4. [数据流与状态管理](#数据流与状态管理)
5. [IPC 通信机制](#ipc-通信机制)
6. [核心组件详解](#核心组件详解)
7. [确定性与可重现性](#确定性与可重现性)

---

## 系统架构概览

Cockpit Simulator 采用多层架构设计，从前端 UI 到后端仿真核心完全解耦：

```
┌─────────────────────────────────────────────────────────────────┐
│                        用户界面层 (UI Layer)                     │
│  React 19 + TypeScript + Vite + Tailwind + Lucide              │
│  ┌─────────────┬─────────────┬─────────────┬─────────────┐     │
│  │ 运行控制    │ 世界视图    │ 事件时间线  │ 智能体追踪  │     │
│  └─────────────┴─────────────┴─────────────┴─────────────┘     │
│  ┌───────────────────────────────────────────────────────┐     │
│  │         状态管理 (useReducer + simulationReducer)     │     │
│  │         客户端 API (runnerClient.ts)                  │     │
│  └───────────────────────────────────────────────────────┘     │
└────────────────────────────┬────────────────────────────────────┘
                             │ Tauri IPC (invoke)
┌────────────────────────────▼────────────────────────────────────┐
│                     原生主机层 (Native Host Layer)              │
│  Tauri 2 Rust Backend                                          │
│  ┌───────────────────────────────────────────────────────┐     │
│  │  runner_commands.rs: 15+ Tauri Command 桥接           │     │
│  │  - validate_scenario, create_simulation_run           │     │
│  │  - start/pause/step/stop_simulation                   │     │
│  │  - approve_action, reject_action, cancel_agent_turn   │     │
│  │  - start_replay, diff_recordings                      │     │
│  │  - get_simulation_events, get_simulation_snapshot     │     │
│  └───────────────────────────────────────────────────────┘     │
│  ┌───────────────────────────────────────────────────────┐     │
│  │  RunnerState: 传输层抽象                               │     │
│  │  - Embedded: 嵌入式 RunnerHandler (开发模式)          │     │
│  │  - Process: TCP 环回 (127.0.0.1:47701, 生产模式)     │     │
│  └───────────────────────────────────────────────────────┘     │
└────────────────────────────┬────────────────────────────────────┘
                             │ JSON-line / Direct Call
┌────────────────────────────▼────────────────────────────────────┐
│                    运行器层 (Runner Layer)                       │
│  cockpit-runner crate                                           │
│  ┌───────────────────────────────────────────────────────┐     │
│  │  RunnerHandler: IPC 调度器                             │     │
│  │  - Session 认证 (token)                               │     │
│  │  - 版本化协议 (IPC_VERSION = 1)                       │     │
│  │  - 游标事件流 (cursor-based streaming)                │     │
│  │  - 持久化录制存储 (SQLite)                            │     │
│  └───────────────────────────────────────────────────────┘     │
│  ┌───────────────────────────────────────────────────────┐     │
│  │  LiveRunDriver: 智能体协调                             │     │
│  │  - 重试 / 熔断策略                                     │     │
│  │  - 规则智能体后备                                      │     │
│  │  - 插件执行调度                                        │     │
│  └───────────────────────────────────────────────────────┘     │
└────────────────────────────┬────────────────────────────────────┘
                             │ Rust API Calls
┌────────────────────────────▼────────────────────────────────────┐
│                智能体运行时层 (Agent Runtime Layer)             │
│  cockpit-agent-runtime crate                                   │
│  ┌───────────────────────────────────────────────────────┐     │
│  │  LocalMcpServer: 仿真工具服务器                        │     │
│  │  - 6 个类型化工具 (observation, action, status...)   │     │
│  │  - 能力执行 (capability enforcement)                  │     │
│  │  - 行动审批流程 (pending/approve/reject)              │     │
│  │  - 敏感数据脱敏 (redaction)                           │     │
│  └───────────────────────────────────────────────────────┘     │
│  ┌───────────────────────────────────────────────────────┐     │
│  │  RuleAgent: 确定性后备智能体                           │     │
│  │  - 烟雾检测 → 引擎关闭规则                            │     │
│  │  - 零外部依赖                                         │     │
│  └───────────────────────────────────────────────────────┘     │
│  ┌───────────────────────────────────────────────────────┐     │
│  │  MultiAgentCoordinator: 多智能体协调                   │     │
│  │  - 稳定优先级排序                                      │     │
│  │  - 重复目标命令拒绝                                    │     │
│  └───────────────────────────────────────────────────────┘     │
│  ┌───────────────────────────────────────────────────────┐     │
│  │  IotaCoreAcpAdapter: 外部 LLM 智能体适配器 (可选)     │     │
│  │  - 观测转提示词                                       │     │
│  │  - 事件映射为脱敏追踪                                  │     │
│  │  - 可取消回合 API                                     │     │
│  └───────────────────────────────────────────────────────┘     │
└────────────────────────────┬────────────────────────────────────┘
                             │ Simulation API
┌────────────────────────────▼────────────────────────────────────┐
│                 仿真核心层 (Simulation Core Layer)              │
│  cockpit-simulation-core crate                                 │
│  ┌───────────────────────────────────────────────────────┐     │
│  │  Simulation: 确定性状态机                              │     │
│  │  - Tick-based 世界推进                                │     │
│  │  - 行动验证与应用                                      │     │
│  │  - 故障注入                                           │     │
│  │  - 影响规则调度                                        │     │
│  │  - 快照哈希计算 (SHA-256)                             │     │
│  └───────────────────────────────────────────────────────┘     │
│  ┌───────────────────────────────────────────────────────┐     │
│  │  WorldSnapshot: 世界状态快照                           │     │
│  │  - Environment (温度、湿度、可见性、烟雾密度...)      │     │
│  │  - Human (stress, fatigue, health, attention)        │     │
│  │  - Device (engine, alarm 状态)                       │     │
│  └───────────────────────────────────────────────────────┘     │
│  ┌───────────────────────────────────────────────────────┐     │
│  │  Action Gateway: 行动执行网关                          │     │
│  │  - 能力检查 (capability check)                        │     │
│  │  - 版本校验 (version mismatch)                        │     │
│  │  - 过期检测 (action expiry)                           │     │
│  │  - 冲突检测 (duplicate target command)                │     │
│  └───────────────────────────────────────────────────────┘     │
│  ┌───────────────────────────────────────────────────────┐     │
│  │  Event System: 事件信封与有效载荷                      │     │
│  │  - 类型化事件 (SmokeDetected, ActionApplied...)      │     │
│  │  - 优先级与序列号                                      │     │
│  │  - 关联 ID 追踪                                       │     │
│  └───────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────────┘

横切关注点 (Cross-cutting Concerns):
┌─────────────────────────────────────────────────────────────────┐
│  cockpit-recording: 录制与回放                                   │
│  - Recording 结构 (schema/runtime/world version)                │
│  - RecordingStore: SQLite 持久化 + 内容寻址 SHA-256             │
│  - Replay 引擎 + Diff 工具                                      │
│  - 有界队列 + 溢出策略                                           │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│  cockpit-plugin: 插件系统                                        │
│  - PluginHost: 清单发现与验证                                   │
│  - StateDiff 门控 (entity/component/version/range)              │
│  - 失败策略 (ignore/pause/fail)                                 │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│  cockpit-evaluation: 评估框架                                    │
│  - 通过/失败评分                                                │
│  - 证据事件追踪                                                 │
│  - 场景特定规则 (如烟雾关机评估)                                │
└─────────────────────────────────────────────────────────────────┘
```

---

## 分层架构详解

### 1. 用户界面层 (UI Layer)

**技术栈**：React 19, TypeScript, Vite 7, Tailwind 4, Lucide Icons

**核心组件**：
- **App.tsx**：主应用容器
  - 使用 `useReducer` 管理全局仿真状态
  - 处理键盘快捷键
  - 初始化连接与重连逻辑
  - 从 localStorage 加载持久化会话

- **SimulationRunControl**：运行控制面板
  - 场景加载与验证
  - 开始/暂停/单步/停止控制
  - 审批模式切换
  - 自动步进功能

- **SimulationWorldView**：世界可视化
  - 实体列表显示
  - 传感器质量指标
  - 环境覆盖层（可见性、烟雾）
  - 实时状态更新

- **SimulationTimeline**：事件时间线
  - 分页事件列表
  - 导出功能 (JSON/CSV)
  - Tick/类型/消息显示

- **SimulationTrace**：智能体追踪
  - 工具调用记录
  - 行动审批 UI
  - 参数/结果脱敏显示
  - 导出追踪数据

- **SimulationReplay**：录制回放
  - 回放启动
  - 录制差异对比
  - 差异报告显示

- **SimulationEvaluation**：评估结果
  - 通过/失败指示器
  - 得分进度条
  - 评估说明

**状态管理**：
```typescript
// simulationReducer.ts
export const initialSimulationModel: SimulationModel = {
  state: "disconnected",
  tick: 0,
  simTimeMs: 0,
  observations: [],
  events: [],
  toolCalls: [],
  actionResults: [],
  serviceConnected: false,
  approvalRequired: false
};

// 20+ action types 处理所有状态转换
type SimulationAction = 
  | { type: "connectRequested" }
  | { type: "connected" }
  | { type: "scenarioReady"; scenario: ScenarioSummary; runId?: string }
  | { type: "runnerEvent"; event: RunnerEvent }
  // ...
```

**客户端 API**：
```typescript
// runnerClient.ts
export interface RunnerClient {
  connect(): Promise<void>;
  validateScenario(path: string): Promise<ScenarioSummary>;
  createRun(path: string): Promise<string>;
  start/pause/step/stop(): Promise<void>;
  approveAction/rejectAction/cancelAgentTurn(): Promise<...>;
  snapshot(cursor?: number): Promise<RunnerEventBatch>;
  // ...
}
```

---

### 2. 原生主机层 (Native Host Layer)

**技术栈**：Tauri 2, Rust

**核心文件**：
- `src-tauri/src/lib.rs`：应用入口，注册 Tauri 命令
- `src-tauri/src/runner_commands.rs`：命令实现

**RunnerState 结构**：
```rust
pub struct RunnerState {
    transport: Mutex<RunnerTransport>,  // 传输层抽象
    token: String,                       // 会话令牌
    sequence: Mutex<u64>,                // 请求序列号
    workspace_root: PathBuf,             // 工作区根目录
}

enum RunnerTransport {
    Embedded(Box<RunnerHandler>),        // 嵌入式模式
    Process { child: Child, address: SocketAddr }, // 进程模式
}
```

**传输层选择**：
- **嵌入式** (dev 默认)：直接调用 `RunnerHandler::dispatch()`
- **进程** (生产推荐)：
  - 通过 `COCKPIT_RUNNER_BIN` 环境变量指定 runner 二进制
  - 启动子进程监听 `127.0.0.1:47701`
  - JSON-line 协议通信
  - 自动重连（断线时重新启动进程）

**关键命令桥接**：
```rust
#[tauri::command]
pub fn validate_scenario(state: tauri::State<RunnerState>, path: String) 
    -> Result<ScenarioSummary, String> {
    let resolved_path = state.resolve_path(&path);
    serde_json::from_value(
        state.dispatch(RunnerCommand::ValidateScenario { path: resolved_path })?
    ).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn step_simulation(state: tauri::State<RunnerState>) 
    -> Result<(), String> {
    state.dispatch(RunnerCommand::StepSimulation).map(|_| ())
}

// ... 15+ 命令
```

**路径解析**：
- 相对路径自动相对于 `workspace_root`
- 绝对路径保持不变
- 支持 dev 模式和打包后的路径差异

---

### 3. 运行器层 (Runner Layer)

**Crate**：`cockpit-runner`

**核心结构**：
```rust
pub struct RunnerHandler {
    session_token: String,
    simulation: Option<Simulation>,
    recording: Option<Recording>,
    server: LocalMcpServer,
    agent: RuleAgent,
    events: Vec<RunnerEvent>,          // 内存事件缓冲 (2048 max)
    next_cursor: u64,
    recording_store: Option<RecordingStore>,
    plugin_host: PluginHost,
    plugin_executors: BTreeMap<String, Box<dyn PluginExecutor>>,
    recording_queue: RecordingQueue,
}
```

**IPC 协议**：
```rust
// proto.rs
pub const IPC_VERSION: u16 = 1;

pub struct RunnerRequest {
    pub version: u16,
    pub session_token: String,
    pub correlation_id: String,
    pub command: RunnerCommand,
}

pub struct RunnerResponse {
    pub version: u16,
    pub correlation_id: String,
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<IpcError>,
}
```

**命令类型**：
```rust
pub enum RunnerCommand {
    ValidateScenario { path: String },
    CreateSimulationRun { path: String },
    ResumeSimulation { scenario_path: String, run_id: String },
    StartSimulation,
    PauseSimulation,
    StepSimulation,
    StopSimulation,
    ApproveAction { request_id: String },
    RejectAction { request_id: String, reason: Option<String> },
    CancelAgentTurn,
    SetApprovalRequired { required: bool },
    GetSimulationSnapshot,
    GetSimulationEvents { cursor: Option<u64> },
    StartReplay { scenario_path: String, recording_path: String },
    DiffRecordings { source_recording_path: String, candidate_recording_path: String },
}
```

**事件流机制**：
```rust
pub enum RunnerEvent {
    SimulationStateChanged { cursor: u64, state: RunStatus, run_id: Option<String> },
    SimulationTickCommitted { cursor: u64, snapshot: WorldSnapshot },
    SimulationEvent { cursor: u64, event: EventEnvelope },
    SimulationToolCall { cursor: u64, trace: ToolCallTrace },
    SimulationActionResult { cursor: u64, result: ActionResult },
    SimulationPluginFailure { cursor: u64, failure: PluginFailureRecord },
    SimulationEvaluationUpdated { cursor: u64, evaluation: Value },
    SimulationError { cursor: u64, error: IpcError },
}
```

**游标恢复**：
- 客户端请求 `GetSimulationEvents { cursor: Some(last_cursor) }`
- 返回 `{ events, nextCursor, firstAvailableCursor, resetRequired }`
- `resetRequired = true` 表示游标失效，需要完整快照

**持久化录制**：
- 通过 `RecordingStore::open(database_path)` 启用
- 每次 `step()` 后调用 `persist_recording()`
- SQLite 存储元数据 + SHA-256 内容寻址 blob
- 支持 `ResumeSimulation` 跨进程恢复

**LiveRunDriver**：
```rust
pub async fn run_live(config: LiveRunConfig) -> anyhow::Result<LiveRunReport> {
    let policy = AgentRuntimePolicy::new(timeout_ms, 1, FallbackPolicy::RuleAgent)
        .with_retry(max_attempts, circuit_failure_threshold);
    let mut driver = LiveAgentDriver::new(policy);

    for _ in 0..config.ticks {
        let step = driver.step(&mut simulation, &mut server, Vec::new(), || {
            async move { backend.run_turn(&observation).await }
        }).await?;
        
        // 记录 disposition (completed/fallback/cancelled)
        evidence.push(LiveTickEvidence { tick, snapshot_hash, disposition });
        recording.push(step);
    }
}
```

---

### 4. 智能体运行时层 (Agent Runtime Layer)

**Crate**：`cockpit-agent-runtime`

**LocalMcpServer**：仿真工具服务器

```rust
pub struct LocalMcpServer {
    action_results: BTreeMap<String, ActionResult>,
    pending_actions: BTreeMap<String, ActionRequest>,
    approval_required: bool,
}

// 6 个类型化工具
pub const TOOL_GET_OBSERVATION: &str = "simulation.get_observation";
pub const TOOL_LIST_VISIBLE_ENTITIES: &str = "simulation.list_visible_entities";
pub const TOOL_INSPECT_SENSOR_QUALITY: &str = "simulation.inspect_sensor_quality";
pub const TOOL_REQUEST_ACTION: &str = "simulation.request_action";
pub const TOOL_GET_ACTION_RESULT: &str = "simulation.get_action_result";
pub const TOOL_GET_RUN_STATUS: &str = "simulation.get_run_status";
```

**工具调用流程**：
```rust
pub fn call(&mut self, simulation: &mut Simulation, request: ToolRequest) 
    -> (ToolResponse, ToolCallTrace) {
    
    // 1. 身份验证
    if request.run_id != simulation.run_id() { /* 拒绝 */ }
    if request.agent_id != simulation.scenario.agent.agent_id { /* 拒绝 */ }
    
    // 2. 分发到具体工具
    let result = self.dispatch(simulation, &request);
    
    // 3. 响应大小限制
    if response_fits(&result) { /* OK */ } else { /* PAYLOAD_TOO_LARGE */ }
    
    // 4. 脱敏
    let trace = ToolCallTrace {
        arguments: redact_json(request.arguments),
        result: redact_json(response_value),
        side_effect: request.tool_name == TOOL_REQUEST_ACTION,
        allowed: /* 根据验证结果 */,
    };
    
    (response, trace)
}
```

**行动审批流程**：
```rust
// 请求行动
fn request_action(&mut self, simulation: &mut Simulation, request: &ToolRequest) 
    -> Result<Value, ToolError> {
    let action = /* 解析参数构建 ActionRequest */;
    
    if self.approval_required {
        let result = ActionResult {
            status: ActionStatus::PendingApproval,
            /* ... */
        };
        self.pending_actions.insert(action.request_id.clone(), action);
        self.action_results.insert(result.request.request_id.clone(), result.clone());
        return Ok(serde_json::to_value(result)?);
    }
    
    // 直接执行
    let result = simulation.submit_action(action);
    Ok(serde_json::to_value(result)?)
}

// 批准行动
pub fn approve_action(&mut self, simulation: &mut Simulation, request_id: &str) 
    -> Result<ActionResult, ToolError> {
    let action = self.pending_actions.remove(request_id)?;
    let result = simulation.submit_action(action);
    self.action_results.insert(result.request.request_id.clone(), result.clone());
    Ok(result)
}

// 拒绝行动
pub fn reject_action(&mut self, simulation: &Simulation, request_id: &str, cancelled: bool) 
    -> Result<ActionResult, ToolError> {
    let action = self.pending_actions.remove(request_id)?;
    let result = ActionResult {
        status: ActionStatus::Rejected,
        error_code: Some(if cancelled { 
            ErrorCode::ActionCancelled 
        } else { 
            ErrorCode::ApprovalDenied 
        }),
        /* ... */
    };
    Ok(result)
}
```

**RuleAgent**：确定性后备智能体

```rust
pub struct RuleAgent {
    shutdown_requested: bool,
}

impl RuleAgent {
    pub fn step_with_state_diffs(&mut self, simulation: &mut Simulation, 
        server: &mut LocalMcpServer, state_diffs: Vec<StateDiff>) 
        -> SimulationResult<StepRecord> {
        
        let observation = simulation.observation();
        
        // 规则：检测到烟雾 → 请求引擎关闭
        if !self.shutdown_requested && observation.alerts.contains(&"smoke-detected".to_string()) {
            let action = ActionRequest {
