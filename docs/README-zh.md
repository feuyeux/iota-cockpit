# Cockpit Simulator 中文文档索引

本目录包含 Cockpit Simulator 项目的完整中文文档。

## 文档列表

### 📖 用户指南

#### [桌面端使用指南](./desktop-user-guide-zh.md)
**目标受众**：终端用户、测试人员、研究人员

**内容概要**：
- 应用启动与界面布局
- 场景管理与加载
- 仿真控制（开始/暂停/单步/停止）
- 世界视图与传感器质量
- 事件时间线查看与导出
- 智能体追踪与行动审批
- 录制回放与差异对比
- 评估结果解读
- 快捷键与高级功能
- 故障排除

**何时阅读**：首次使用应用时，或需要了解具体功能时

---

### 🏗️ 架构文档

#### [分层架构与请求链路流程](./architecture-and-flow-zh.md)
**目标受众**：开发者、架构师、代码审查者

**内容概要**：
- 系统架构概览（5层架构）
- 各层详细设计与职责
- 前端层：React 组件与状态管理
- Tauri 原生层：IPC 桥接与传输抽象
- 运行器层：事件流与持久化
- 智能体运行时层：工具服务与审批流程
- 仿真核心层：确定性状态机
- 请求链路完整流程
- IPC 通信机制详解
- 数据结构定义
- 安全与权限控制
- 确定性保证机制
- 性能优化策略
- 错误处理与恢复

**何时阅读**：需要理解系统设计、修改代码、或进行技术决策时

---

#### [可视化流程图](./visual-flow-diagrams-zh.md)
**目标受众**：所有技术人员

**内容概要**：
- 系统架构层次图（Mermaid）
- 单步执行完整请求链路序列图
- 智能体工具调用流程图
- 行动审批状态机
- 录制回放与差异对比流程
- 事件流与游标恢复机制
- 持久化录制与进程重启恢复
- 多智能体协调流程
- 插件执行与失败处理
- 完整生命周期状态机

**何时阅读**：需要快速理解系统流程、调试问题、或向他人解释系统时

---

## 文档导航建议

### 我是新用户
1. 先阅读 **[桌面端使用指南](./desktop-user-guide-zh.md)** 的"启动应用"和"界面布局"部分
2. 跟随"场景管理"和"仿真控制"部分进行实践
3. 遇到问题时查看"故障排除"部分

### 我是开发者（前端）
1. 阅读 **[分层架构与请求链路流程](./architecture-and-flow-zh.md)** 的"前端层"部分
2. 参考 **[可视化流程图](./visual-flow-diagrams-zh.md)** 理解数据流
3. 查看代码：`apps/cockpit-desktop/src/` 目录

### 我是开发者（后端/Rust）
1. 阅读 **[分层架构与请求链路流程](./architecture-and-flow-zh.md)** 的后端层部分
2. 参考 **[可视化流程图](./visual-flow-diagrams-zh.md)** 的"单步执行完整请求链路"
3. 查看代码：`crates/` 目录

### 我需要调试问题
1. 查看 **[可视化流程图](./visual-flow-diagrams-zh.md)** 找到相关流程
2. 阅读 **[分层架构与请求链路流程](./architecture-and-flow-zh.md)** 的"错误处理与恢复"部分
3. 参考 **[桌面端使用指南](./desktop-user-guide-zh.md)** 的"故障排除"部分

### 我要进行系统集成
1. 阅读 **[分层架构与请求链路流程](./architecture-and-flow-zh.md)** 的"IPC 通信机制"
2. 查看 **[可视化流程图](./visual-flow-diagrams-zh.md)** 的请求链路
3. 参考代码：`crates/cockpit-runner/src/ipc/proto.rs`

---

## 快速参考

### 核心概念

| 概念 | 说明 | 相关文档 |
|------|------|----------|
| **Tick** | 仿真的基本时间单位，每个 tick 是一次完整的世界状态推进 | 用户指南 §5, 架构文档 §6 |
| **WorldSnapshot** | 某个 tick 的完整世界状态快照，包含环境、人类、设备状态 | 架构文档 §4.2 |
| **快照哈希** | 快照的 SHA-256 哈希值，用于确定性验证和回放 | 架构文档 §6.1 |
| **游标** | 事件流的位置标记，用于增量拉取和重连恢复 | 架构文档 §3, 流程图 §6 |
| **行动审批** | 人工审核智能体的副作用操作的流程 | 用户指南 §7, 流程图 §4 |
| **录制** | 完整的 tick 序列记录，包含所有事件和状态变化 | 用户指南 §8, 架构文档 §3 |
| **回放** | 根据录制文件逐 tick 重现仿真过程 | 用户指南 §8, 流程图 §5 |
| **插件** | 外部状态修改器，可以在 tick 提交前注入状态差异 | 架构文档 §2.6, 流程图 §9 |
| **影响规则** | 计划的世界状态变化规则，在特定 tick 自动应用 | 架构文档 §5 |

### 关键命令

```bash
# 启动桌面应用（开发模式）
cd apps/cockpit-desktop
npm run dev

# 启动原生 Tauri 应用
npm run tauri:dev

# 打包生产版本
npm run tauri:build

# 验证场景文件
cargo run -p cockpit-runner -- validate scenarios/smoke-in-cockpit.yaml

# 运行仿真（CLI）
cargo run -p cockpit-runner -- run scenarios/smoke-in-cockpit.yaml --ticks 80

# 运行实时智能体仿真
cargo run -p cockpit-runner -- run-live scenarios/smoke-in-cockpit.yaml --ticks 80

# 运行测试
cargo test --workspace
npm test  # 在 apps/cockpit-desktop 目录

# 格式化和 Lint
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

### 目录结构

```
iota-cockpit-simulator/
├── apps/
│   └── cockpit-desktop/          # Tauri 2 + React 桌面应用
│       ├── src/                   # React 前端代码
│       ├── src-tauri/             # Tauri Rust 原生代码
│       └── scenarios/             # 场景文件（开发用）
├── crates/
│   ├── cockpit-agent-runtime/    # 智能体运行时
│   ├── cockpit-evaluation/       # 评估框架
│   ├── cockpit-plugin/           # 插件系统
│   ├── cockpit-recording/        # 录制与回放
│   ├── cockpit-runner/           # 运行器（IPC 服务器）
│   ├── cockpit-scenario/         # 场景解析
│   └── cockpit-simulation-core/  # 仿真核心
├── scenarios/                     # 示例场景
├── tests/                         # 集成测试
└── docs/                          # 文档（本目录）
```

---

## 贡献文档

如果您发现文档错误或希望改进文档，请：

1. 在 GitHub 上提交 Issue 或 Pull Request
2. 遵循现有文档的格式和风格
3. 确保代码示例可运行
4. 更新相关的图表和索引

---

## 英文文档

英文版本的文档位于：
- 主 README：`/README.md`
- 完整需求与架构：`/doc/001.md`
- 各 crate 的 README：`/crates/*/README.md`（如果有）

---

## 许可证

本项目及其文档遵循项目根目录的 LICENSE 文件。

---

**最后更新时间**：2026-07-13  
**文档版本**：1.0.0  
**对应代码版本**：Phase 0-2 实现完成
