# Cockpit Simulator 可视化流程图

## 1. 系统架构层次图

```mermaid
graph TB
    subgraph UI["前端层 - React + TypeScript"]
        A1[运行控制]
        A2[世界视图]
        A3[事件时间线]
        A4[智能体追踪]
        A5[录制回放]
        A6[评估显示]
        A7[状态管理<br/>simulationReducer]
        A8[API客户端<br/>runnerClient]
    end
    
    subgraph Tauri["Tauri原生层 - Rust"]
        B1[runner_commands.rs<br/>15+ Tauri Commands]
        B2[RunnerState<br/>传输层抽象]
        B3[Embedded Mode]
        B4[Process Mode<br/>TCP 127.0.0.1:47701]
    end
    
    subgraph Runner["运行器层 - cockpit-runner"]
        C1[RunnerHandler<br/>IPC调度器]
        C2[LiveRunDriver<br/>智能体驱动]
        C3[事件流缓冲<br/>2048 events]
        C4[持久化存储<br/>SQLite + SHA-256]
    end
    
    subgraph AgentRT["智能体运行时层 - cockpit-agent-runtime"]
        D1[LocalMcpServer<br/>6个仿真工具]
        D2[RuleAgent<br/>规则智能体]
        D3[MultiAgentCoordinator<br/>多智能体协调]
        D4[行动审批流程]
    end
    
    subgraph Core["仿真核心层 - cockpit-simulation-core"]
        E1[Simulation<br/>确定性状态机]
        E2[WorldSnapshot<br/>世界状态]
        E3[Action Gateway<br/>行动验证]
        E4[Event System<br/>事件信封]
        E5[Influence System<br/>影响规则]
    end
    
    subgraph XCutting["横切关注点"]
        F1[cockpit-recording<br/>录制/回放]
        F2[cockpit-plugin<br/>插件系统]
        F3[cockpit-evaluation<br/>评估框架]
    end
    
    A1 --> A8
    A2 --> A8
    A3 --> A8
    A4 --> A8
    A5 --> A8
    A6 --> A8
    A7 --> A8
    
    A8 -->|Tauri IPC| B1
    B1 --> B2
    B2 --> B3
    B2 --> B4
    
    B3 --> C1
    B4 --> C1
    C1 --> C2
    C1 --> C3
    C1 --> C4
    
    C2 --> D1
    C2 --> D2
    C2 --> D3
    D1 --> D4
    
    D1 --> E1
    D2 --> E1
    D3 --> E1
    
    E1 --> E2
    E1 --> E3
    E1 --> E4
    E1 --> E5
    
    C1 -.-> F1
    C2 -.-> F2
    C1 -.-> F3
    
    style UI fill:#e1f5ff
    style Tauri fill:#fff4e1
    style Runner fill:#ffe1f5
    style AgentRT fill:#e1ffe1
    style Core fill:#f5e1ff
    style XCutting fill:#ffffcc
```

## 2. 单步执行完整请求链路

```mermaid
sequenceDiagram
    participant User as 用户
    participant UI as SimulationRunControl
    participant Client as runnerClient
    participant Tauri as Tauri IPC
    participant Cmd as runner_commands
    participant State as RunnerState
    participant Handler as RunnerHandler
    participant Driver as LiveRunDriver
    participant Server as LocalMcpServer
    participant Sim as Simulation
    participant Reducer as simulationReducer
    
    User->>UI: 点击 Step 按钮
    UI->>Client: runCommand(runnerClient.step)
    Client->>Tauri: invoke("step_simulation")
    Tauri->>Cmd: step_simulation(state)
    Cmd->>State: dispatch(StepSimulation)
    
    alt Embedded Mode
        State->>Handler: handler.dispatch(request)
    else Process Mode
        State->>State: TCP JSON-line 发送
        State-->>Handler: 接收响应
    end
    
    Handler->>Handler: 验证版本和token
    Handler->>Handler: 运行插件
    Handler->>Driver: 协调智能体回合
    Driver->>Server: 提供工具访问
    Server->>Sim: 调用仿真API
    
    Handler->>Sim: commit_step()
    
    Sim->>Sim: 1. 应用故障注入
    Sim->>Sim: 2. 应用影响规则
    Sim->>Sim: 3. 应用环境演变
    Sim->>Sim: 4. 应用待处理行动
    Sim->>Sim: 5. 应用插件状态差异
    Sim->>Sim: 6. 递增tick和version
    Sim->>Sim: 7. 计算快照哈希
    
    Sim-->>Handler: 返回 StepRecord
    Handler->>Handler: 发射事件到缓冲区
    Handler->>Handler: 持久化录制
    Handler-->>State: RunnerResponse
    State-->>Cmd: Result<Value>
    Cmd-->>Tauri: Result<(), String>
    Tauri-->>Client: Promise resolves
    
    Client->>Client: syncEvents()
    Client->>Tauri: snapshot(lastCursor)
    Tauri->>Handler: GetSimulationEvents
    Handler-->>Client: EventBatch
    
    Client->>Reducer: dispatch(runnerEvent)
    Reducer->>Reducer: 更新状态
    Reducer-->>UI: 触发重新渲染
    UI->>User: 显示更新后的状态
```

## 3. 智能体工具调用流程

```mermaid
sequenceDiagram
    participant Agent as 智能体
    participant Server as LocalMcpServer
    participant Sim as Simulation
    participant Gateway as Action Gateway
    participant Handler as RunnerHandler
    
    Agent->>Server: call(ToolRequest)<br/>simulation.request_action
    Server->>Server: 验证 run_id + agent_id
    Server->>Server: dispatch(simulation, request)
    
    alt approval_required = true
        Server->>Server: 创建 ActionResult<br/>{ status: PendingApproval }
        Server->>Server: pending_actions.insert()
        Server-->>Agent: 返回 PendingApproval
        Server->>Handler: 等待审批
    else approval_required = false
        Server->>Sim: submit_action(action)
        Sim->>Gateway: validate_action()
        
        Gateway->>Gateway: 1. 检查能力授权
        Gateway->>Gateway: 2. 检查过期时间
        Gateway->>Gateway: 3. 检查状态版本
        Gateway->>Gateway: 4. 检查行动冲突
        Gateway->>Gateway: 5. 检查前置条件
        
        alt 验证通过
            Gateway->>Sim: pending_actions.push()
            Sim-->>Server: ActionResult { status: Applied }
        else 验证失败
            Sim-->>Server: ActionResult { status: Rejected, error_code }
        end
        
        Server-->>Agent: 返回结果
    end
    
    Server->>Server: 创建 ToolCallTrace<br/>（脱敏处理）
    Server->>Handler: 记录追踪
```

## 4. 行动审批流程

```mermaid
stateDiagram-v2
    [*] --> Requested: 智能体请求行动
    Requested --> PendingApproval: approval_required=true
    Requested --> Validating: approval_required=false
    
    PendingApproval --> Approved: 用户点击批准
    PendingApproval --> Rejected: 用户点击拒绝
    PendingApproval --> Cancelled: 取消智能体回合
    
    Approved --> Validating: 移除pending<br/>提交到Simulation
    
    Validating --> Applied: 验证通过
    Validating --> Rejected: 验证失败
    
    Applied --> [*]: 行动将在下个tick执行
    Rejected --> [*]: 发射ActionRejected事件
    Cancelled --> [*]: 发射ActionCancelled事件
```

## 5. 录制回放与差异对比

```mermaid
flowchart TD
    A[用户选择录制文件] --> B[加载场景和录制]
    B --> C[创建新Simulation实例]
    C --> D[simulation.start]
    
    D --> E{遍历录制的每个tick}
    E --> F[提取recorded_actions]
    E --> G[提取recorded_state_diffs]
    
    F --> H[simulation.step_with_recorded_inputs]
    G --> H
    
    H --> I[计算新快照哈希]
    I --> J{哈希匹配?}
    
    J -->|是| K[发射TickCommitted事件]
    J -->|否| L[抛出ReplayHashMismatch错误]
    
    K --> M{还有更多tick?}
    M -->|是| E
    M -->|否| N[完成回放]
    
    N --> O[设置状态为Completed]
    O --> P[前端显示回放结果]
    
    L --> Q[前端显示错误]
    
    style J fill:#ffe1e1
    style L fill:#ff6b6b
    style N fill:#e1ffe1
```

## 6. 事件流与游标恢复机制

```mermaid
sequenceDiagram
    participant Client as 前端客户端
    participant Handler as RunnerHandler
    participant Buffer as 事件缓冲区<br/>(2048 max)
    participant Store as RecordingStore<br/>(SQLite)
    
    Note over Handler: 仿真运行中
    Handler->>Handler: step() 执行
    Handler->>Buffer: emit(TickCommitted, cursor=100)
    Handler->>Buffer: emit(SimulationEvent, cursor=101)
    Handler->>Buffer: emit(ToolCall, cursor=102)
    Handler->>Store: persist_recording()
    
    Note over Client: 客户端定期拉取
    Client->>Handler: GetSimulationEvents<br/>{ cursor: 95 }
    Handler->>Buffer: events_after(95)
    Buffer-->>Handler: [events 96-102]
    Handler-->>Client: { events, nextCursor: 103 }
    
    Client->>Client: dispatch(runnerEvent)
    Client->>Client: lastCursor = 102
    
    Note over Client: 连接断开
    Client->>Client: 检测断线
    Client->>Client: exponentialBackoff重连
    
    Note over Client: 重连成功
    Client->>Handler: GetSimulationEvents<br/>{ cursor: 102 }
    
    alt 游标仍在缓冲区内
        Handler->>Buffer: events_after(102)
        Buffer-->>Handler: [events 103-150]
        Handler-->>Client: { events, resetRequired: false }
        Client->>Client: 增量应用事件
    else 游标已过期（被清理）
        Handler-->>Client: { resetRequired: true,<br/>firstAvailableCursor: 1000 }
        Client->>Handler: GetSimulationSnapshot
        Handler->>Store: 加载最新快照
        Handler-->>Client: 完整WorldSnapshot
        Client->>Client: 重置状态
        Client->>Handler: GetSimulationEvents<br/>{ cursor: 1000 }
        Handler-->>Client: { events 1001+ }
    end
```

## 7. 持久化录制与进程重启恢复

```mermaid
flowchart TD
    A[Runner启动] --> B{配置了<br/>recording_db?}
    B -->|是| C[RecordingStore::open]
    B -->|否| D[仅内存模式]
    
    C --> E[handler.new_persistent]
    D --> E
    
    E --> F{接收CreateSimulationRun<br/>还是ResumeSimulation?}
    
    F -->|Create| G[创建新Simulation]
    G --> H[创建新Recording]
    
    F -->|Resume| I[从SQLite加载Recording]
    I --> J[创建新Simulation]
    J --> K[逐tick重放]
    
    K --> L[step_with_recorded_inputs<br/>for each tick]
    L --> M[恢复到最后一个tick]
    
    H --> N[开始运行]
    M --> N
    
    N --> O[每个step后]
    O --> P[recording.push]
    O --> Q[recording_store.save]
    
    Q --> R{进程崩溃?}
    R -->|是| S[重启进程]
    S --> A
    
    R -->|否| T[继续运行]
    T --> O
    
    style I fill:#e1f5ff
    style K fill:#e1f5ff
    style M fill:#b3e5ff
    style Q fill:#ffe1e1
```

## 8. 多智能体协调流程

```mermaid
flowchart TD
    A[MultiAgentCoordinator.step] --> B[收集所有智能体的行动请求]
    
    B --> C[按优先级排序<br/>priority + agent_id]
    
    C --> D{遍历排序后的请求}
    
    D --> E{目标是否已被占用?}
    
    E -->|否| F[标记目标为已占用]
    F --> G[提交行动到Simulation]
    G --> H[记录ActionResult]
    
    E -->|是| I[拒绝行动<br/>error: ActionConflict]
    I --> H
    
    H --> J{还有更多请求?}
    J -->|是| D
    J -->|否| K[返回AgentActionBatch]
    
    K --> L[应用到仿真tick]
    
    style C fill:#e1ffe1
    style E fill:#fffacd
    style I fill:#ffe1e1
    style G fill:#b3ffb3
```

## 9. 插件执行与失败处理

```mermaid
sequenceDiagram
    participant Handler as RunnerHandler
    participant Host as PluginHost
    participant Executor as PluginExecutor
    participant Policy as PluginPolicy
    participant Sim as Simulation
    
    Handler->>Handler: step() 开始
    Handler->>Host: execute(snapshot)
    
    loop 每个已加载的插件
        Host->>Executor: execute(snapshot)
        
        alt 执行成功
            Executor-->>Host: Vec<StateDiff>
            Host->>Policy: validate_state_diffs()
            
            alt 验证通过
                Policy-->>Host: 通过
            else 验证失败
                Policy-->>Host: PluginFailure<br/>{ decision: Ignore/Pause/Fail }
            end
        else 执行异常
            Executor-->>Host: Error
            Host->>Policy: 查询失败策略
            Policy-->>Host: PluginFailure<br/>{ decision: Ignore/Pause/Fail }
        end
    end
    
    Host-->>Handler: (Vec<StateDiff>, Vec<PluginFailure>)
    
    Handler->>Handler: 检查失败策略
    
    alt 包含 FailRun
        Handler->>Sim: simulation.fail()
    else 包含 PauseRun
        Handler->>Sim: simulation.pause()
    else 仅 Ignore
        Handler->>Handler: 记录但继续
    end
    
    Handler->>Sim: commit_step(state_diffs)
    Handler->>Handler: emit(PluginFailure事件)
```

## 10. 完整生命周期状态机

```mermaid
stateDiagram-v2
    [*] --> Disconnected: 应用启动
    
    Disconnected --> Connecting: connect()
    Connecting --> ConnectedIdle: 连接成功
    Connecting --> Disconnected: 连接失败
    
    ConnectedIdle --> ScenarioLoading: 加载场景
    ScenarioLoading --> ScenarioInvalid: 验证失败
    ScenarioLoading --> RunCreating: 验证通过
    
    ScenarioInvalid --> ScenarioLoading: 重试
    
    RunCreating --> Ready: 创建运行成功
    RunCreating --> Failed: 创建失败
    
    Ready --> Running: start()
    Running --> Paused: pause()
    Running --> Degraded: 传感器质量下降
    Running --> Failed: 执行失败
    Running --> Stopped: stop()
    
    Paused --> Running: start()
    Paused --> Stopped: stop()
    
    Degraded --> Running: 质量恢复
    Degraded --> Paused: pause()
    Degraded --> Failed: 执行失败
    
    Replaying --> Completed: 回放完成
    Replaying --> Failed: 回放失败
    
    Completed --> [*]
    Stopped --> [*]
    Failed --> [*]
    
    ConnectedIdle --> Replaying: startReplay()
```

---

## 使用说明

这些图表使用 Mermaid 语法编写，可以在支持 Mermaid 的 Markdown 渲染器中直接查看，例如：

- **GitHub**：直接在 `.md` 文件中显示
- **VS Code**：安装 Markdown Preview Mermaid Support 插件
- **在线工具**：https://mermaid.live/

如需导出为图片，可以使用 `mermaid-cli` 工具：

```bash
npm install -g @mermaid-js/mermaid-cli
mmdc -i visual-flow-diagrams-zh.md -o diagrams.pdf
```
