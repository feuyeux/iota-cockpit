# Cockpit Simulator 中文文档索引

## 主文档

### [Cockpit Desktop 仿真统一指南](./cockpit-desktop-simulation-guide-zh.md)

本文是桌面端唯一主维护入口，基于最新 Cockpit 与 iota-core 代码整合以下内容：

- 启动、1600 × 900 分区聚焦工作台、Live/Replay、快捷键与故障排除；
- 场景控制、世界视图与实时活动流的常驻协作，以及按需展开的评测/叙事洞察；
- 10 个标杆场景、14 域矩阵、动作契约和评测阈值；
- React → Tauri → Runner → iota-core → `hermes acp` 的真实链路；
- Desktop Embedded/Process 通信、JSON-line IPC、事件游标和重连原理图；
- Hermes 子进程、ACP session 复用、逐人物 prompt 与取消时序；
- 世界模型、感知延迟、Action Gateway、录制回放、插件、安全和原生验收清单。

> 最新 Desktop Live 只使用真实 `iota-core-acp` 模型路径；`RuleAgent` 仅保留于 CLI、Runner 合约和测试，不是 Desktop fallback。

## 专题文档

- [NPC 世界建模](./npc.md)

**最后更新时间**：2026-07-14
**统一文档版本**：4.1.0
