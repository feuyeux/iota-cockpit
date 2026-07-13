# Cockpit Simulator 桌面端使用指南

## 目录

1. [概述](#概述)
2. [启动应用](#启动应用)
3. [界面布局](#界面布局)
4. [场景管理](#场景管理)
5. [仿真控制](#仿真控制)
6. [世界视图](#世界视图)
7. [事件时间线](#事件时间线)
8. [智能体追踪](#智能体追踪)
9. [录制回放](#录制回放)
10. [评估结果](#评估结果)
11. [快捷键](#快捷键)
12. [高级功能](#高级功能)

---

## 概述

Cockpit Simulator 是一个基于 Tauri 2 + React 19 + TypeScript 构建的独立驾驶舱世界仿真桌面应用。它提供了完整的智能体运行时环境，支持：

- **确定性仿真**：基于 tick 的世界状态机，完全可重现
- **智能体集成**：支持规则智能体和实时 LLM 智能体（可选）
- **录制与回放**：每个 tick 的完整状态快照和事件记录
- **行动审批流程**：支持人工审核智能体的副作用操作
- **插件系统**：可扩展的外部状态修改器
- **评估框架**：基于场景的自动化评分

---

## 启动应用

### 开发模式

```bash
cd apps/cockpit-desktop
npm install
npm run dev -- --host 127.0.0.1 --port 15342
```

在浏览器中打开 <http://127.0.0.1:15342>

### 原生桌面模式（Tauri）

```bash
cd apps/cockpit-desktop

# 开发模式（使用嵌入式 runner）
npm run tauri:dev

# 使用外部 runner 进程
export COCKPIT_RUNNER_BIN=/path/to/cockpit-runner
npm run tauri:dev

# 打包生产版本
npm run tauri:build
```

**注意**：
- 不设置 `COCKPIT_RUNNER_BIN` 时使用嵌入式处理器
- 设置后会通过 TCP 环回（127.0.0.1:47701）连接独立进程
- 生产构建会自动将 runner 打包为 sidecar 二进制文件

---

## 界面布局

应用界面分为以下几个主要区域：

```
┌──────────────────────────────────────────────────────────┐
│  Header: 状态栏、连接状态、当前 tick、重连按钮、帮助    │
├──────────────────────────────────────────────────────────┤
│  左侧面板                  │  中央区域                   │
│  ┌─────────────────────┐  │  ┌───────────────────────┐  │
│  │ 场景管理            │  │  │ 世界视图              │  │
│  │ - 加载场景          │  │  │ - 实体状态            │  │
│  │ - 场景路径          │  │  │ - 传感器质量          │  │
│  ├─────────────────────┤  │  │ - 可见性/烟雾密度      │  │
│  │ 运行控制            │  │  └───────────────────────┘  │
│  │ - 开始/暂停         │  │  ┌───────────────────────┐  │
│  │ - 单步执行          │  │  │ 事件时间线            │  │
│  │ - 停止              │  │  │ - tick / 事件类型      │  │
│  ├─────────────────────┤  │  │ - 事件消息            │  │
│  │ 录制回放            │  │  │ - 分页/导出           │  │
│  │ - 回放录制文件      │  │  └───────────────────────┘  │
│  │ - 对比差异          │  │                              │
│  ├─────────────────────┤  │                              │
│  │ 评估结果            │  │                              │
│  │ - 通过/失败         │  │                              │
│  │ - 得分进度条        │  │                              │
│  └─────────────────────┘  │                              │
├──────────────────────────────────────────────────────────┤
│  底部面板                                                │
│  ┌────────────────────────────────────────────────────┐  │
│  │ 智能体追踪                                          │  │
│  │ - 工具调用记录                                      │  │
│  │ - 行动请求与结果                                    │  │
│  │ - 审批/拒绝操作（需开启审批模式）                   │  │
│  │ - 分页/导出                                         │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

---

## 场景管理

### 加载场景

1. **输入场景路径**
   - 在"场景管理"区域的文本框中输入场景文件路径
   - 支持相对路径（相对于工作区根目录）和绝对路径
   - 默认路径：`scenarios/smoke-in-cockpit.yaml`

2. **浏览文件**
   - 点击"文件夹"图标打开文件选择器
   - 筛选 `.yaml` 和 `.yml` 文件

3. **加载操作**
   - 点击"加载"按钮（或使用快捷键）
   - 应用会验证场景文件的有效性
   - 验证成功后创建新的仿真运行实例

### 场景文件结构

场景文件是 YAML 格式，包含：
- **id**：场景唯一标识符
- **seed**：随机数种子（保证确定性）
- **clock**：时钟配置（tickMs、tickLimit）
- **environment**：初始环境状态（温度、湿度、可见性、烟雾密度等）
- **pilot/engine/alarm**：实体初始状态
- **agent**：智能体授权信息
- **faults**：计划的故障注入
- **influences**：外部影响规则
- **shutdown_deadline_ticks**：评估截止 tick

示例：
```yaml
id: smoke-in-cockpit
seed: 42
clock:
  tickMs: 1000
  tickLimit: 100
agent:
  agentId: cockpit-agent
  capabilities:
    - engineShutdown
    - alarmActivate
faults:
  - atTick: 5
    target: cabin
    faultType: smokeDetected
```

---

## 仿真控制

### 运行状态

仿真运行有以下状态：
- **disconnected**：未连接到 runner 服务
- **connecting**：正在连接
- **connectedIdle**：已连接，但未加载场景
- **ready**：场景已加载，准备运行
- **running**：仿真正在运行
- **paused**：已暂停
- **degraded**：降级模式（传感器质量下降）
- **replaying**：正在回放录制
- **completed**：仿真完成
- **stopped**：用户手动停止
- **failed**：仿真失败

### 控制按钮

#### 开始 (Play)
- **条件**：状态为 `ready`、`paused` 或 `degraded`
- **功能**：启动仿真，开始执行 tick
- **快捷键**：无

#### 暂停 (Pause)
- **条件**：状态为 `running`
- **功能**：暂停仿真，保留当前状态
- **快捷键**：`Space` 键

#### 单步执行 (Step)
- **条件**：状态为 `running`、`paused` 或 `ready`
- **功能**：执行单个 tick，然后自动暂停
- **快捷键**：`S` 键
- **高级功能**：支持自动步进模式（持续单步执行）

#### 停止 (Stop)
- **条件**：状态为 `running` 或 `paused`
- **功能**：停止仿真，释放资源
- **快捷键**：无

### 自动步进模式

在运行控制面板中：
1. 勾选"自动步进"复选框
2. 设置步进间隔（默认 500ms）
3. 点击"开始"，仿真将自动执行单步操作
4. 取消勾选或手动暂停以停止

---

## 世界视图

世界视图显示当前仿真状态的可视化表示。

### 实体列表

左侧显示所有被监控的实体：
- **cabin**：驾驶舱环境
- **pilot-1**：飞行员状态
- **engine-1**：引擎状态
- **alarm-1**：警报器状态

### 传感器质量指标

显示最新观测的传感器质量：
- **Visibility**：可见性质量（0-100%）
- **Audio**：音频质量（0-100%）
- **Confidence**：整体置信度（0-100%）
- **Degraded**：质量是否降级（黄色边框提示）

### 可视化区域

中央区域显示：
- **环境覆盖层**：根据可见性动态调整透明度
- **实体边界框**：以不同颜色显示关键实体
- **烟雾效果**：当检测到烟雾时显示

**注意**：地面真实状态（ground truth）被隐藏，智能体只能看到传感器观测结果。

---

## 事件时间线

事件时间线显示仿真过程中发生的所有事件。

### 事件结构

每个事件包含：
- **Tick**：事件发生的 tick 编号
- **事件类型**：如 `SmokeDetected`、`ActionApplied`、`FaultInjected` 等
- **消息**：事件的详细描述
- **来源**：事件产生的源实体或系统组件

### 分页导航

- 每页显示固定数量的事件（配置在 `APP_CONFIG.EVENTS_PER_PAGE`）
- 使用左右箭头按钮在页面间导航
- 显示当前页码和总页数

### 导出功能

点击下载图标，选择导出格式：
- **JSON**：完整的事件数据结构
- **CSV**：表格格式，便于在 Excel 中分析

导出的文件会自动下载到本地，文件名包含时间戳。

---

## 智能体追踪

智能体追踪面板显示智能体的所有工具调用和行动结果。

### 工具调用记录

显示智能体调用的模拟工具：
- **simulation.get_observation**：获取传感器观测
- **simulation.list_visible_entities**：列出可见实体
- **simulation.inspect_sensor_quality**：检查传感器质量
- **simulation.request_action**：请求执行行动
- **simulation.get_action_result**：查询行动结果
- **simulation.get_run_status**：获取运行状态

每条记录包含：
- **Call ID**：调用唯一标识
- **工具名称**
- **参数**：传入的参数（敏感信息已脱敏）
- **结果**：返回值（敏感信息已脱敏）
- **Tick**：调用发生的 tick

### 行动审批流程

当启用"审批模式"时：

1. **待审批行动**
   - 智能体请求的副作用操作（如关闭引擎）会进入 `pendingApproval` 状态
   - 在追踪面板中显示为黄色高亮

2. **审批操作**
   - 点击"✓"按钮批准行动
   - 点击"✗"按钮拒绝行动
   - 批准后行动立即执行，拒绝后返回 `ActionRejected` 事件

3. **取消行动**
   - 点击"取消智能体回合"按钮可取消所有待审批行动
   - 所有待处理的行动会被标记为 `ActionCancelled`

### 导出追踪数据

- **JSON**：完整的工具调用和行动结果
- **CSV**：表格格式

---

## 录制回放

录制回放功能允许您重现之前的仿真运行并对比结果。

### 回放录制文件

1. **输入录制路径**
   - 在"录制回放"区域输入录制文件路径（`.json` 文件）
   - 支持相对路径和绝对路径

2. **浏览文件**
   - 点击文件夹图标选择录制文件

3. **开始回放**
   - 点击"回放"按钮
   - 系统会加载录制，并逐 tick 重现
   - 回放过程中状态为 `replaying`

### 对比录制差异

**功能**：对比两个录制文件，检测确定性分歧。

1. **输入源录制路径**（source recording）
2. **输入候选录制路径**（candidate recording）
3. **点击"对比"按钮**

**差异报告**包含：
- **equivalent**：两个录制是否等价
- **firstDivergence**：首次出现差异的 tick
- **sourceMetrics / candidateMetrics**：统计信息
  - ticks：总 tick 数
  - events：事件数
  - toolCalls：工具调用数
  - actionResults：行动结果数
  - stateDiffs：状态差异数
- **tickDifferences**：逐 tick 的差异详情
  - snapshotHash 不匹配
  - 事件/工具调用/行动结果不匹配
  - 状态差异不匹配

**用例**：
- 验证代码更改是否破坏确定性
- 检测插件或影响规则的副作用
- 回归测试

---

## 评估结果

评估面板显示基于场景目标的自动化评分。

### 评估指标

- **Passed / Failed**：是否通过评估
- **Score**：得分（0.0 - 1.0）
- **Explanation**：评估说明
- **Evidence Event IDs**：支持评估结论的事件 ID
- **First Failure Tick**：首次失败的 tick（如果有）

### 烟雾关机评估（示例）

对于 `smoke-in-cockpit` 场景：
- **目标**：在截止时间前关闭引擎
- **评分规则**：
  - 100%：在烟雾检测后立即关闭引擎
  - 降低：根据延迟时间线性递减
  - 0%：超过截止时间或引擎未关闭

---

## 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Space` | 暂停仿真 |
| `S` | 单步执行（在 `running` 或 `paused` 状态） |
| `?` | 显示键盘快捷键帮助 |
| `Esc` | 关闭帮助面板 |

**注意**：当输入框、文本区域或选择框处于焦点状态时，快捷键不生效。

---

## 高级功能

### 恢复仿真

从之前的运行恢复（需要持久化录制存储）：

```typescript
await runnerClient.resume(scenarioPath, runId);
```

**条件**：
- runner 配置了 `--recording-db` 参数
- 运行 ID 对应的录制数据存在

**用途**：
- 进程崩溃后恢复
- 继续长时间运行的仿真
- 调试特定 tick 状态

### 设置审批模式

```typescript
await runnerClient.setApprovalRequired(true);
```

**开启后**：
- 所有副作用工具调用（`simulation.request_action`）进入 `pendingApproval` 状态
- 需要人工审批或拒绝
- 智能体被阻塞直到操作完成

**用途**：
- 人机协作决策
- 高风险操作的安全控制
- 教学和演示

### 导出完整运行数据

导出包含所有事件、工具调用和行动结果的完整运行数据：

```typescript
import { exportTracesAsJSON, exportEventsAsJSON } from "./utils/export";

// 导出智能体追踪
exportTracesAsJSON(model.toolCalls);

// 导出事件时间线
exportEventsAsJSON(model.events);

// 导出行动结果
exportActionResultsAsJSON(model.actionResults);
```

### 事件游标与重连恢复

应用使用游标机制处理断线重连：

1. **游标跟踪**
   - `model.lastCursor` 记录最后接收的事件游标
   - 每个事件携带唯一递增的游标编号

2. **重连流程**
   ```typescript
   const batch = await runnerClient.snapshot(model.lastCursor);
   if (batch.resetRequired) {
     // 游标已失效，重新获取完整快照
     const snapshot = await runnerClient.simulationSnapshot();
     dispatch({ type: "snapshotReset", snapshot });
   } else {
     // 增量更新
     for (const event of batch.events) {
       dispatch({ type: "runnerEvent", event });
     }
   }
   ```

3. **重连指数退避**
   ```typescript
   await exponentialBackoff(async () => {
     await runnerClient.connect();
     await syncEvents();
   });
   ```

**配置**（在 `utils/reconnect.ts`）：
- 初始延迟：100ms
- 最大延迟：5000ms
- 最大尝试次数：10

### 自定义场景

创建自定义场景文件：

1. 复制现有场景模板
2. 修改实体初始状态
3. 添加故障注入计划
4. 配置智能体授权
5. 设置评估参数

参考 `scenarios/smoke-in-cockpit.yaml` 获取完整示例。

### 插件集成

（高级功能，需要 Rust 编程）

插件可以在每个 tick 执行前注入状态差异（StateDiff）：

```rust
pub trait PluginExecutor {
    fn execute(&mut self, snapshot: &WorldSnapshot) -> Vec<StateDiff>;
}
```

插件失败策略：
- **Ignore**：记录错误但继续运行
- **PauseRun**：暂停仿真
- **FailRun**：使仿真失败

---

## 故障排除

### 连接失败

**症状**：顶部显示"disconnected"状态，黄色连接图标

**解决方案**：
1. 检查 runner 服务是否正在运行
2. 点击"重连"按钮
3. 检查控制台错误日志
4. 确认 `COCKPIT_RUNNER_BIN` 环境变量（如果使用外部进程）

### 场景验证失败

**症状**：加载场景后显示"scenarioInvalid"错误

**解决方案**：
1. 确认场景文件路径正确
2. 验证 YAML 语法
3. 检查必需字段是否存在
4. 使用 CLI 验证：
   ```bash
   cargo run -p cockpit-runner -- validate scenarios/your-scenario.yaml
   ```

### 录制队列溢出

**症状**：事件流中出现 `RECORDING_QUEUE_OVERFLOW` 错误

**解决方案**：
1. 暂停或停止仿真
2. 检查录制存储是否配置正确
3. 调整录制队列策略（在 runner 配置中）
4. 减少仿真复杂度或 tick 频率

### 性能问题

**症状**：仿真运行缓慢，UI 响应延迟

**解决方案**：
1. 关闭实时智能体（使用规则智能体）
2. 减少事件缓冲区大小
3. 禁用不必要的插件
4. 使用单步执行模式
5. 在生产构建中运行（非开发模式）

---

## 系统要求

- **操作系统**：macOS、Windows、Linux
- **Node.js**：18.0 或更高
- **Rust**：1.70 或更高（仅用于构建）
- **内存**：建议 4GB 以上
- **磁盘空间**：录制文件可能占用大量空间

---

## 参考资料

- 架构文档：`docs/architecture-and-flow-zh.md`
- API 参考：`crates/cockpit-*/README.md`
- 场景示例：`scenarios/`
- 测试用例：`tests/`
- 完整需求：`doc/001.md`

---

## 版本历史

- **v0.1.0**：初始发布，Phase 0-2 功能
- 支持确定性仿真、录制回放、智能体运行时
- Tauri 2 桌面客户端
- 插件系统和评估框架

---

## 许可证与贡献

参见项目根目录的 LICENSE 和 CONTRIBUTING.md 文件。

---

**提示**：本指南假设您已熟悉基本的仿真和智能体概念。如需了解底层架构和技术细节，请参考架构文档。
