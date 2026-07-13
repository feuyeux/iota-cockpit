# Cockpit Simulator 架构与请求流程图

## 1. 系统分层架构图

```
┌──────────────────────────────────────────────────────────────────┐
│                      前端层 (Frontend Layer)                      │
│        React 19 + TypeScript + Vite + Tailwind                   │
├──────────────────────────────────────────────────────────────────┤
│  运行控制 │ 世界视图 │ 时间线 │ 智能体追踪 │ 回放 │ 评估         │
├──────────────────────────────────────────────────────────────────┤
│  状态管理: useReducer(simulationReducer, initialModel)           │
│  API 客户端: runnerClient.ts                                     │
└────────────────────┬─────────────────────────────────────────────┘
                     │ Tauri IPC (invoke)
┌────────────────────▼─────────────────────────────────────────────┐
│                  Tauri 原生层 (Native Host)                       │
│              apps/cockpit-desktop/src-tauri                       │
├──────────────────────────────────────────────────────────────────┤
│  runner_commands.rs: 15+ 命令桥接                                │
│  - validate_scenario / create_simulation_run                     │
│  - start/pause/step/stop_simulation                              │
│  - approve_action / reject_action / cancel_agent_turn            │
│  - get_simulation_events / get_simulation_snapshot               │
├──────────────────────────────────────────────────────────────────┤
│  RunnerState (传输层抽象)                                         │
│  ├─ Embedded: 嵌入式 RunnerHandler (开发模式)                    │
│  └─ Process: TCP 127.0.0.1:47701 (生产模式)                      │
└────────────────────┬─────────────────────────────────────────────┘
                     │ JSON-line 协议 / 直接调用
┌────────────────────▼─────────────────────────────────────────────┐
│                    运行器层 (Runner Layer)                        │
│              crates/cockpit-runner                                │
├──────────────────────────────────────────────────────────────────┤
│  RunnerHandler: IPC 协议处理与调度                                │
│  - Session 认证 + 版本控制 (IPC_VERSION=1)                       │
│  - 事件流缓冲 (2048 events, 游标分页)                            │
│  - 持久化录制 (SQLite + SHA-256 内容寻址)                         │
├──────────────────────────────────────────────────────────────────┤
│  LiveRunDriver: 智能体驱动协调                                    │
│  - 超时/重试/熔断策略                                             │
│  - 规则后备 + 插件调度                                            │
└────────────────────┬─────────────────────────────────────────────┘
                     │ Rust API
┌────────────────────▼─────────────────────────────────────────────┐
│               智能体运行时层 (Agent Runtime)                      │
│           crates/cockpit-agent-runtime                            │
├──────────────────────────────────────────────────────────────────┤
│  LocalMcpServer: 6 个仿真工具                                     │
│  - get_observation / list_visible_entities                       │
│  - inspect_sensor_quality / request_action                       │
│  - get_action_result / get_run_status                            │
├──────────────────────────────────────────────────────────────────┤
│  行动审批流程: pending → approve/reject                            │
│  RuleAgent: 烟雾检测 → 引擎关闭规则                               │
│  MultiAgentCoordinator: 多智能体优先级仲裁                        │
└────────────────────┬─────────────────────────────────────────────┘
                     │ Simulation API
┌────────────────────▼─────────────────────────────────────────────┐
│                 仿真核心层 (Simulation Core)                      │
│          crates/cockpit-simulation-core                           │
├──────────────────────────────────────────────────────────────────┤
│  Simulation: 确定性 Tick 状态机                                   │
│  - 行动验证与应用 (Action Gateway)                                │
│  - 故障注入调度 (Fault Injection)                                 │
│  - 影响规则执行 (Influence System)                                │
│  - 快照哈希计算 (SHA-256)                                         │
├──────────────────────────────────────────────────────────────────┤
│  WorldSnapshot: 环境 + 人类 + 设备状态                            │
│  Event System: 类型化事件信封                                     │
│  Observation: 传感器观测与质量                                    │
└──────────────────────────────────────────────────────────────────┘

横切关注点:
  • cockpit-recording: 录制/回放/差异对比
  • cockpit-plugin: 插件系统与状态差异
  • cockpit-evaluation: 评估框架
  • cockpit-scenario: YAML 场景解析
```

## 2. 请求链路流程图

### 2.1 用户点击"单步执行"完整流程

```
[用户] 点击 Step 按钮
   │
   ▼
[SimulationRunControl.tsx]
   │ runCommand(runnerClient.step)
   ▼
[runnerClient.ts]
   │ invoke("step_simulation")
   ▼
[Tauri IPC 层]
   │ 序列化请求 → WebView → Rust
   ▼
[runner_commands.rs]
   │ step_simulation(state: State<RunnerState>)
   │ state.dispatch(RunnerCommand::StepSimulation)
   ▼
[RunnerState 传输层]
   │ 选择传输方式:
   ├─ Embedded: handler.dispatch(request)
   └─ Process: TCP JSON-line 发送/接收
   ▼
[RunnerHandler.dispatch()]
   │ 1. 验证 IPC 版本
   │ 2. 验证 session token
   │ 3. 匹配 RunnerCommand::StepSimulation
   ▼
[RunnerHandler.step()]
   │ 1. 运行插件: plugin_host.execute()
   │ 2. 智能体回合: agent.step_with_state_diffs()
   │    └─ LocalMcpServer 提供工具
   │ 3. 提交 tick: simulation.commit_step()
   │ 4. 记录到 recording 和 queue
   │ 5. 持久化: recording_store.save()
   │ 6. 发射事件: emit(TickCommitted/Event/ToolCall/ActionResult)
   ▼
[Simulation.commit_step()]
   │ 1. 应用故障注入 (faults)
   │ 2. 应用影响规则 (influences)
   │ 3. 应用环境演变
   │ 4. 应用待处理行动 (pending_actions)
   │ 5. 应用插件状态差异 (state_diffs)
   │ 6. 递增 tick 和 version
   │ 7. 计算快照哈希
   │ 8. 生成事件信封
   ▼
[返回 StepRecord]
   │ { tick, snapshot_hash, events, observation, 
   │   action_results, tool_calls, state_diffs, plugin_failures }
   ▼
[RunnerHandler] 发射事件到内存缓冲
   │ events.push(RunnerEvent::SimulationTickCommitted { cursor, snapshot })
   │ events.push(RunnerEvent::SimulationEvent { cursor, event })
   │ events.push(RunnerEvent::SimulationToolCall { cursor, trace })
   │ events.push(RunnerEvent::SimulationActionResult { cursor, result })
   │ next_cursor += events.len()
   ▼
[RunnerResponse] 返回成功结果
   │ { version, correlation_id, ok: true, result: { tick, snapshotHash, status } }
   ▼
[runner_commands.rs] 反序列化并返回
   │
   ▼
[Tauri IPC] 发送 Response → WebView
   │
   ▼
[runnerClient.step()] Promise resolves
   │
   ▼
[useRunner.ts] syncEvents()
   │ const batch = await runnerClient.snapshot(model.lastCursor)
   │ for (event of batch.events) dispatch({ type: "runnerEvent", event })
   ▼
[simulationReducer] 状态更新
   │ case "runnerEvent":
   │   ├─ SimulationTickCommitted: 更新 tick, snapshot
   │   ├─ SimulationEvent: 添加到 events 数组
   │   ├─ SimulationToolCall: 添加到 toolCalls 数组
   │   └─ SimulationActionResult: 添加到 actionResults 数组
   ▼
[React 组件] 重新渲染
   ├─ SimulationWorldView: 显示新 snapshot
   ├─ SimulationTimeline: 显示新 events
   ├─ SimulationTrace: 显示新 toolCalls
   └─ Header: 更新 tick 显示
```

### 2.2 智能体工具调用流程

```
[智能体] 调用 simulation.request_action
   │
   ▼
[LocalMcpServer.call()]
   │ 1. 构建 ToolRequest { call_id, run_id, agent_id, tool_name, arguments }
   │ 2. 验证身份: run_id 匹配 + agent_id 授权
   │ 3. 调用 dispatch(simulation, request)
   ▼
[LocalMcpServer.request_action()]
   │ 解析参数: target, command, expected_state_version, expires_at_tick
   │
   ├─ approval_required = true?
   │  │ YES: 创建 ActionResult { status: PendingApproval }
   │  │      pending_actions.insert(call_id, action)
   │  │      返回 PendingApproval 结果
   │  └─ NO:  simulation.submit_action(action)
   │          返回 Applied/Rejected 结果
   ▼
[Simulation.submit_action()]
   │ validate_action(): 检查
   │ ├─ 能力授权 (capability check)
   │ ├─ 过期时间 (expiry check)
   │ ├─ 状态版本 (version mismatch)
   │ ├─ 行动冲突 (duplicate target)
   │ └─ 前置条件 (precondition: 如引擎是否开启)
   │
   ├─ 通过: pending_actions.push(request)
   │         返回 ActionResult { status: Applied }
   └─ 失败: 返回 ActionResult { status: Rejected, error_code }
   ▼
[ToolResponse + ToolCallTrace]
   │ ToolResponse: { run_id, tick, result, error }
   │ ToolCallTrace: { 
   │   call_id, tool_name, arguments (redacted), 
   │   result (redacted), side_effect: true, allowed: true 
   │ }
   ▼
[返回给智能体 + 记录追踪]
```

### 2.3 行动审批流程

```
[前端] 用户点击"批准"按钮
   │
   ▼
[SimulationTrace.tsx]
   │ approve_action(requestId)
   ▼
[runnerClient.approveAction(requestId)]
   │ invoke("approve_action", { requestId })
   ▼
[runner_commands::approve_action]
   │ dispatch(RunnerCommand::ApproveAction { request_id })
   ▼
[RunnerHandler.approve_action()]
   │ server.approve_action(&mut simulation, request_id)
   ▼
[LocalMcpServer.approve_action()]
   │ 1. 从 pending_actions 移除
   │ 2. simulation.submit_action(action)
   │ 3. action_results.insert(result)
   ▼
[Simulation.submit_action()]
   │ validate_action() + pending_actions.push()
   │ 返回 ActionResult { status: Applied }
   ▼
[RunnerHandler] 
   │ emit(RunnerEvent::SimulationActionResult { cursor, result })
   │ 返回 RunnerResponse { ok: true, result }
   ▼
[前端] syncEvents()
   │ dispatch({ type: "runnerEvent", event: ActionResult })
   │ SimulationTrace 组件移除待审批项，显示已批准状态
```


### 2.4 录制回放流程

```
[用户] 点击"回放"按钮，输入录制路径
   │
   ▼
[SimulationReplay.tsx]
   │ runnerClient.startReplay(scenarioPath, recordingPath)
   ▼
[RunnerHandler.start_replay()]
   │ 1. 加载场景: load_scenario(scenario_path)
   │ 2. 读取录制: fs::read + serde_json::from_slice<Recording>
   │ 3. 验证兼容性: replay_recording(scenario, recording)
   │ 4. 创建 Simulation::new(run_id, scenario)
   │ 5. simulation.start()
   ▼
[逐 tick 重现]
   │ for source_tick in recording.ticks {
   │   actions = recorded_actions_by_tick[tick]
   │   state_diffs = recorded_state_diffs_by_tick[tick]
   │   
   │   step = simulation.step_with_recorded_inputs(actions, state_diffs)
   │   
   │   emit(TickCommitted)
   │   emit(SimulationEvent for each event)
   │   emit(SimulationToolCall for each tool_call)
   │   emit(SimulationActionResult for each result)
   │ }
   ▼
[simulation.status = Completed]
   │ emit(SimulationStateChanged { state: Completed })
   ▼
[前端] 接收事件流，逐 tick 渲染
   │ 状态显示为 "replaying" → "completed"
```

### 2.5 录制差异对比流程

```
[用户] 输入源录制 + 候选录制路径，点击"对比"
   │
   ▼
[SimulationReplay.tsx]
   │ runnerClient.diffRecordings(sourcePath, candidatePath)
   ▼
[RunnerHandler.diff_recordings()]
   │ 1. 读取两个录制文件
   │ 2. 调用 diff_recordings(source, candidate)
   ▼
[cockpit-recording::diff_recordings()]
   │ 对比每个 tick:
   │ ├─ snapshot_hash 是否匹配
   │ ├─ events 数量和内容
   │ ├─ tool_calls 数量和内容
   │ ├─ action_results 数量和内容
   │ └─ state_diffs 数量和内容
   │
   │ 记录首次分歧点: firstDivergence { tick, ... }
   │ 计算指标: sourceMetrics, candidateMetrics
   ▼
[返回 RecordingDiff]
   │ { 
   │   equivalent: bool,
   │   firstDivergence: Option<{ tick, ... }>,
   │   sourceMetrics: { ticks, events, toolCalls, ... },
   │   candidateMetrics: { ticks, events, toolCalls, ... },
   │   tickDifferences: Vec<TickDiff>
   │ }
   ▼
[前端] dispatch({ type: "replayDiffUpdated", report })
   │ SimulationReplay 组件显示差异报告
   │ ├─ 绿色: equivalent = true
   │ └─ 黄色: 显示首次分歧 tick + 指标对比
```

## 3. 事件流与游标机制

### 3.1 事件发射与缓冲

```rust
// RunnerHandler
impl RunnerHandler {
    fn emit(&mut self, event: RunnerEvent) {
        let cursor = self.next_cursor;
        self.next_cursor += 1;
        
        // 为事件附加游标
        let event_with_cursor = match event {
            RunnerEvent::SimulationTickCommitted { snapshot, .. } => 
                RunnerEvent::SimulationTickCommitted { cursor, snapshot },
            // ... 其他事件类型
        };
        
        // 环形缓冲区，保留最新 2048 个事件
        self.events.push(event_with_cursor);
        if self.events.len() > MAX_EVENT_HISTORY {
            self.events.remove(0);
        }
    }
}
```

### 3.2 游标分页与重连

```typescript
// 客户端重连逻辑
async function reconnect() {
  dispatch({ type: "connectRequested" });
  
  const result = await exponentialBackoff(async () => {
    await runnerClient.connect();
    
    // 使用 lastCursor 增量获取事件
    const batch = await runnerClient.snapshot(model.lastCursor);
    
    if (batch.resetRequired) {
      // 游标失效（事件已被清理），需要完整快照
      const snapshot = await runnerClient.simulationSnapshot();
      dispatch({ type: "snapshotReset", snapshot, cursor: batch.firstAvailableCursor - 1 });
    }
    
    // 增量应用事件
    for (const event of batch.events) {
      dispatch({ type: "runnerEvent", event });
    }
  });
  
  if (result.success) {
    dispatch({ type: "connected" });
  }
}
```

### 3.3 持久化录制与进程重启恢复

```rust
// runner serve 模式启动时
let recording_db = "/tmp/cockpit-runner-recording.sqlite";
let mut handler = RunnerHandler::new_persistent(token, recording_db)?;

// 每次 step 后
fn step(&mut self) -> HandlerResult {
    let step = /* ... 执行 tick ... */;
    
    // 推入录制队列
    self.recording_queue.push(step.clone());
    
    // 持久化到 SQLite
    self.persist_recording()?;
    
    // 发射事件
    self.emit(TickCommitted);
    // ...
}

// 进程重启后恢复
fn resume_run(&mut self, scenario_path: &str, run_id: &str) -> HandlerResult {
    let store = self.recording_store.as_ref()?;
    let recording = store.load(run_id)?;
    
    // 重放所有 tick 以恢复状态
    for source_tick in &recording.ticks {
        let step = simulation.step_with_recorded_inputs(actions, state_diffs)?;
        // 重新发射事件
    }
}
```

## 4. 数据结构关键定义

### 4.1 前端状态模型

```typescript
interface SimulationModel {
  state: RunState;  // disconnected | ready | running | paused | ...
  tick: number;
  simTimeMs: number;
  speed: number;
  
  scenario?: ScenarioSummary;
  runId?: string;
  snapshot?: WorldSnapshot;
  
  observations: Observation[];
  events: SimulationEvent[];
  toolCalls: ToolCallTrace[];
  actionResults: ActionResult[];
  
  serviceConnected: boolean;
  approvalRequired: boolean;
  error?: SimulationError;
  evaluation?: EvaluationResult;
  replayDiff?: RecordingDiff;
  
  lastCursor?: number;  // 事件游标
}
```

### 4.2 后端核心结构

```rust
// 世界快照
pub struct WorldSnapshot {
    pub run_id: String,
    pub tick: u64,
    pub sim_time_ms: u64,
    pub version: u64,  // 状态版本（用于乐观锁）
    pub environment: EnvironmentState,
    pub pilot: HumanState,
    pub engine: DeviceState,
    pub alarm: AlarmState,
}

// Tick 记录
pub struct StepRecord {
    pub tick: u64,
    pub snapshot_hash: String,  // SHA-256
    pub events: Vec<EventEnvelope>,
    pub observation: Observation,
    pub action_results: Vec<ActionResult>,
    pub tool_calls: Vec<ToolCallTrace>,
    pub state_diffs: Vec<StateDiff>,
    pub plugin_failures: Vec<PluginFailureRecord>,
    pub errors: Vec<String>,
    pub fallback: Option<String>,
}

// 录制结构
pub struct Recording {
    pub schema_version: u32,
    pub runtime_contract_version: u32,
    pub world_model_version: u32,
    pub application_commit: String,
    pub plugin_hashes: Vec<String>,
    pub run_id: String,
    pub scenario_hash: String,
    pub seed: u64,
    pub clock: ClockConfig,
    pub ticks: Vec<StepRecord>,
}
```

## 5. 安全与权限控制

### 5.1 能力检查

```rust
// AgentGrant 定义智能体授权
pub struct AgentGrant {
    pub agent_id: String,
    pub capabilities: Vec<Command>,
}

impl AgentGrant {
    pub fn allows(&self, agent_id: &str, command: &Command) -> bool {
        self.agent_id == agent_id && self.capabilities.contains(command)
    }
}

// Action Gateway 验证
fn validate_action(&mut self, request: &ActionRequest) -> ActionResult {
    let authorized = scenario.agent.allows(&request.agent_id, &request.command);
    
    let error_code = if !authorized {
        Some(ErrorCode::CapabilityDenied)
    } else if request.expires_at_tick < snapshot.tick {
        Some(ErrorCode::ActionExpired)
    } else if request.expected_state_version != snapshot.version {
        Some(ErrorCode::VersionMismatch)
    } else {
        None
    };
    
    // ...
}
```

### 5.2 敏感数据脱敏

```rust
pub fn redact_json(mut value: Value) -> Value {
    redact_json_in_place(&mut value);
    value
}

fn redact_json_in_place(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                if sensitive_key(key) {
                    *val = Value::String("[REDACTED]".to_string());
                } else {
                    redact_json_in_place(val);
                }
            }
        }
        Value::Array(arr) => arr.iter_mut().for_each(redact_json_in_place),
        _ => {}
    }
}

fn sensitive_key(key: &str) -> bool {
    matches!(
        key.to_lowercase().as_str(),
        "api_key" | "token" | "secret" | "password" | "credential" | "reasoning"
    )
}

// ToolCallTrace 自动应用脱敏
let trace = ToolCallTrace {
    arguments: redact_json(request.arguments),
    result: redact_json(response.result),
    // ...
};
```

## 6. 确定性保证机制

### 6.1 快照哈希计算

```rust
impl WorldSnapshot {
    pub fn content_hash(&self) -> SimulationResult<String> {
        // 序列化为规范 JSON（字段有序）
        let canonical = serde_json::to_string(self)?;
        
        // SHA-256 哈希
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let hash = hasher.finalize();
        
        Ok(format!("{:x}", hash))
    }
}
```

### 6.2 确定性排序

```rust
// 智能体行动按优先级和 agent_id 稳定排序
impl MultiAgentCoordinator {
    pub fn arbitrate(&self, requests: Vec<ActionRequest>) -> Vec<ActionRequest> {
        let mut sorted = requests;
        sorted.sort_by(|a, b| {
            self.priority(&a.agent_id)
                .cmp(&self.priority(&b.agent_id))
                .then(a.agent_id.cmp(&b.agent_id))
        });
        sorted
    }
}

// 事件序列号保证顺序
pub struct EventEnvelope {
    pub event_id: String,
    pub tick: u64,
    pub priority: u32,
    pub sequence: u64,  // 单调递增
    // ...
}
```

### 6.3 录制回放验证

```rust
pub fn replay_recording(
    run_id: impl Into<String>,
    scenario: SimulationScenario,
    recording: &Recording,
) -> SimulationResult<Recording> {
    let mut simulation = Simulation::new(run_id, scenario);
    simulation.start()?;
    
    let mut replay = Recording::new(simulation.run_id(), &simulation.scenario);
    
    for tick_record in &recording.ticks {
        let actions = /* 提取已录制的行动 */;
        let state_diffs = /* 提取已录制的状态差异 */;
        
        let step = simulation.step_with_recorded_inputs(actions, state_diffs)?;
        
        // 验证快照哈希匹配
        if step.snapshot_hash != tick_record.snapshot_hash {
            return Err(SimulationError::ReplayHashMismatch {
                tick: step.tick,
                expected: tick_record.snapshot_hash.clone(),
                actual: step.snapshot_hash,
            });
        }
        
        replay.push(step);
    }
    
    Ok(replay)
}
```

## 7. 性能优化策略

### 7.1 事件流分页

- 内存缓冲区限制：2048 个事件（环形队列）
- 游标分页：客户端按需拉取增量事件
- 重连时自动同步：指数退避重试

### 7.2 录制队列

```rust
pub struct RecordingQueue {
    capacity: usize,
    buffer: Vec<StepRecord>,
    policy: RecordingQueuePolicy,
}

pub enum RecordingQueuePolicy {
    FailRun,      // 溢出时使仿真失败
    PauseRun,     // 溢出时暂停仿真
    DropOldest,   // 丢弃最旧的记录
}
```

### 7.3 内容寻址存储

```rust
// PayloadStore: SHA-256 去重
pub fn store_payload(&mut self, data: &[u8]) -> Result<(String, usize)> {
    let hash = sha256_hex(data);
    let size = data.len();
    
    // 检查是否已存在
    if self.exists(&hash)? {
        return Ok((hash, size));
    }
    
    // 写入文件系统
    let path = self.payload_path(&hash);
    fs::write(&path, data)?;
    
    Ok((hash, size))
}
```

## 8. 错误处理与恢复

### 8.1 IPC 错误传播

```rust
pub struct IpcError {
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
    pub run_id: Option<String>,
    pub tick: Option<String>,
    pub correlation_id: String,
}

// 错误码标准化
pub enum ErrorCode {
    CapabilityDenied,
    ActionExpired,
    VersionMismatch,
    ActionConflict,
    UnknownTarget,
    DeviceUnpowered,
    PreconditionFailed,
    ActionCancelled,
    ApprovalDenied,
}
```

### 8.2 插件失败策略

```rust
pub enum PluginFailurePolicy {
    Ignore,     // 记录但继续
    PauseRun,   // 暂停仿真
    FailRun,    // 使仿真失败
}

// 应用到 step
fn step(&mut self) -> HandlerResult {
    let (plugin_diffs, plugin_failures) = self.run_plugins(&simulation);
    
    for failure in &plugin_failures {
        if failure.decision == PluginFailurePolicy::PauseRun {
            simulation.status = RunStatus::Paused;
        }
        if failure.decision == PluginFailurePolicy::FailRun {
            simulation.fail();
        }
    }
}
```

### 8.3 重连指数退避

```typescript
async function exponentialBackoff<T>(
  operation: () => Promise<T>,
  maxAttempts = 10,
  initialDelay = 100,
  maxDelay = 5000
): Promise<{ success: boolean; result?: T; error?: Error; attempts: number }> {
  let delay = initialDelay;
  
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      const result = await operation();
      return { success: true, result, attempts: attempt };
    } catch (error) {
      if (attempt === maxAttempts) {
        return { success: false, error: error as Error, attempts: attempt };
      }
      
      await new Promise(resolve => setTimeout(resolve, delay));
      delay = Math.min(delay * 2, maxDelay);
    }
  }
}
```

---

## 总结

Cockpit Simulator 采用清晰的分层架构，每一层都有明确的职责：

1. **前端层**：用户交互与状态展示
2. **Tauri 原生层**：跨平台桥接与进程管理
3. **运行器层**：IPC 协议、事件流、持久化
4. **智能体运行时层**：工具服务、审批流程、后备策略
5. **仿真核心层**：确定性状态机、行动验证、快照哈希

请求链路从用户点击到仿真 tick 完成，经过多层严格验证和转换，最终以事件流的形式回传前端，实现了：

- **确定性**：种子固定 + 快照哈希验证
- **可重现性**：完整录制 + 逐 tick 回放
- **安全性**：能力检查 + 审批流程 + 数据脱敏
- **可恢复性**：游标分页 + 持久化存储 + 进程重启恢复
- **可扩展性**：插件系统 + 影响规则 + 多智能体协调

这种架构设计使得系统既能满足复杂的智能体仿真需求，又能保持高可靠性和可维护性。
