# Cockpit Desktop 分层架构与 Sidecar

## 一句话说明

**Sidecar（伴随进程）**是由 Cockpit Desktop 原生层启动并管理的独立本地可执行程序。它不是浏览器插件、云服务，也不是另一个桌面窗口。它将模拟运行和独立评测从 WebView 与桌面进程中隔离出来；Desktop 通过本机受鉴权的 IPC 与它通信。

## 分层架构图

```mermaid
flowchart TB
    User[用户]

    subgraph Desktop[桌面应用进程: cockpit-desktop]
        direction TB
        Web[React WebView<br/>场景控制、世界视图、活动流、评测报告]
        Host[Tauri Rust 原生层<br/>命令路由、进程管理、文件访问]
        Web <--> |Tauri commands| Host
    end

    subgraph Sidecars[本机 Sidecar 独立进程]
        direction LR
        Runner[cockpit-runner<br/>仿真服务]
        Evaluator[cockpit-evaluator<br/>独立评测]
    end

    subgraph Runtime[运行时与领域层]
        direction LR
        Agent[cockpit-agent-runtime<br/>ACP 人物代理与 MCP 工具桥]
        Simulation[cockpit-simulation-core<br/>权威世界状态与动作校验]
        Scenario[cockpit-scenario<br/>场景 YAML 解析]
        Recording[cockpit-recording<br/>Recording 与 SQLite]
    end

    subgraph External[用户配置的本地后端]
        Hermes[Hermes Desktop Agent<br/>ACP 模型后端]
    end

    subgraph Data[本地资源与数据]
        Public[scenarios/*.yaml<br/>公开初始场景]
        Private[evaluations/private/*<br/>私有评测 rubric]
        Db[(Recording SQLite<br/>检查点与事件)]
    end

    User --> Web
    Host <-->|版本化、会话鉴权 IPC| Runner
    Host -->|启动并接收报告| Evaluator
    Runner --> Scenario
    Scenario --> Public
    Runner --> Simulation
    Runner --> Agent
    Agent <-->|ACP + 原生 MCP| Hermes
    Runner --> Recording
    Recording <--> Db
    Evaluator -->|只读| Recording
    Evaluator --> Private
    Evaluator -->|报告| Host
```

## Sidecar 分工

| 进程 | 何时启动 | 职责 | 可访问的数据 |
| --- | --- | --- | --- |
| `cockpit-runner` | Desktop 创建或恢复仿真时 | 载入场景、维护权威世界状态、执行动作校验、驱动 Live ACP 人物回合、保存 Recording | 公开场景、模拟状态、Recording SQLite；**不读取私有 rubric** |
| `cockpit-evaluator` | 用户请求评测时 | 只读 Recording，按私有 rubric 生成 `pass`、`fail` 或 `inconclusive` 报告 | Recording、私有 rubric；**不修改仿真世界** |

`cockpit-runner` 是仿真的 Ground Truth 所有者。前端展示状态和发送命令，但不直接修改世界。`cockpit-evaluator` 则是独立评测平面，避免运行中的模拟进程既执行又给自己评分。

## 一次 Live 运行的数据流

```mermaid
sequenceDiagram
    participant W as React WebView
    participant H as Tauri 原生层
    participant R as cockpit-runner sidecar
    participant A as 人物 ACP 代理
    participant M as Hermes Desktop Agent
    participant D as Recording SQLite

    W->>H: 创建 Live Run / 单步
    H->>R: 受鉴权 IPC 请求
    R->>A: 为每个人物驱动一个回合
    A->>M: ACP prompt 与 MCP 工具调用
    M-->>A: toolCall 或 final JSON
    A->>R: 经过动作网关验证的结果
    R->>D: 保存 tick、事件与检查点
    R-->>H: IPC 响应与事件游标
    H-->>W: 更新世界视图和活动流
```

