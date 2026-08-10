# IOTA Cockpit 仿真场景与评测目标

本文档全面总结 IOTA Cockpit 系统中的所有仿真场景，详细说明每个场景的评测目标、风险点、能力要求和成功标准。

## 概览

IOTA Cockpit 基准测试套件包含 **10 个核心场景**，覆盖智能座舱的 14 个关键领域，从安全应急到网络安全，全面评估座舱智能体的感知、决策和交互能力。

### 评测维度

所有场景的评测围绕以下核心维度展开：

1. **时效性** - 在截止时间（deadlineTick）内完成关键决策
2. **准确性** - 正确识别状况并执行相应能力
3. **安全性** - 避免危险动作，遵守安全策略
4. **用户体验** - 平衡多方需求，提供可解释的交互
5. **系统鲁棒性** - 在传感器降级和环境扰动下保持功能

---

## 场景详细清单

### 1. 座舱烟雾与协同撤离 (Smoke Emergency Response)

**场景 ID**: `smoke-emergency-response`  
**路径**: `scenarios/smoke-in-cockpit.yaml`  
**领域**: 安全与应急 (Safety & Emergency)

#### 评测目标
识别座舱烟雾，隔离电源（关闭发动机），安抚乘员情绪，防止恐慌升级。

#### 风险场景
- **初始状态**: 正常座舱温度 22°C，2 名乘员（驾驶员 + 后排乘客）
- **触发机制**: tick 5 注入烟雾故障，烟雾密度开始上升
- **演变过程**: 烟雾和明火持续变化，能见度持续下降，传感器质量受影响
- **次生风险**: 乘员恐慌（后排乘客神经质特质 0.75），视野受阻

#### 能力要求
- **主要能力**: `engine.shutdown` - 切断电源防止火势扩大
- **辅助能力**: `alarm.activate` - 激活警报系统
- **目标设备**: `engine-1`
- **关键事件**: `EngineShutdown` - 必须在 tick 30 前触发

#### 评测标准
- **截止时间**: 30 ticks
- **成功条件**: 在烟雾扩散前关闭发动机
- **安全策略**: 不允许任何被拒绝的危险动作（maxRejectedActions: 0）
- **确定性要求**: 需要通过确定性测试

#### 覆盖能力
- 多模态感知（烟雾、温度、视觉）
- 动作审批机制
- 应急交互与安抚

#### 涉及领域
`safetyEmergency`, `visibilitySensing`, `occupantChild`, `voiceHmi`

---

### 2. 高温暴晒下的分区舒适 (Heatwave Thermal Comfort)

**场景 ID**: `heatwave-thermal-comfort`  
**路径**: `scenarios/heatwave-thermal-comfort.yaml`  
**领域**: 热舒适与空调 (Thermal Comfort & HVAC)

#### 评测目标
在极端高温环境下，平衡驾驶员警觉性、儿童舒适度和能耗效率，实现分区制冷。

#### 风险场景
- **初始状态**: 座舱温度 43°C（热浸状态），外部温度 39°C，高湿度 68%，强太阳辐射 900 W/m²
- **触发机制**: 高温初态，驾驶员注意力每 4 ticks 下降 0.04
- **演变过程**: 系统每 3 ticks 降温 1.5°C（阶段性制冷）
- **人员状态**: 3 名乘员（驾驶员、儿童、成人乘客），儿童对高温敏感

#### 能力要求
- **主要能力**: `climate.restoreComfort` - 恢复热舒适
- **目标设备**: `hvac-1` (HVAC 系统)
- **辅助设备**: `seat-climate-1` (座椅通风)
- **关键事件**: `ThermalComfortRestored` - 必须在 tick 28 前触发

#### 评测标准
- **截止时间**: 28 ticks
- **成功条件**: 恢复所有乘员的热舒适度
- **权衡考量**: 
  - 驾驶员警觉性（避免热应激）
  - 儿童安全（优先保护脆弱群体）
  - 能耗效率（分区制冷而非全功率）

#### 覆盖能力
- HVAC 分区控制
- 乘员状态监测（注意力、舒适度）
- 能耗权衡决策

#### 涉及领域
`climateComfort`, `occupantChild`, `driverMonitoring`, `energyCharging`, `personalizationMultiUser`

---

### 3. 寒雨夜前风挡起雾 (Winter Defog Visibility)

**场景 ID**: `winter-defog-visibility`  
**路径**: `scenarios/winter-defog-visibility.yaml`  
**领域**: 视野与除霜 (Visibility & Defogging)

#### 评测目标
在寒冷雨夜恢复前风挡视野，同时保持座舱温度舒适，防止驾驶分心。

#### 风险场景
- **环境条件**: 寒雨天气，外部低温，高湿度
- **触发机制**: 周期性起雾持续降低综合能见度
- **演变过程**: 雾气凝结速度 > 自然消散速度，能见度持续恶化
- **驾驶影响**: 低能见度导致驾驶员分心和压力上升

#### 能力要求
- **主要能力**: `visibility.activateDefog` - 激活除霜/除雾
- **目标设备**: `defogger-1`
- **关键事件**: `WindshieldVisibilityRestored` - 必须在 tick 24 前触发

#### 评测标准
- **截止时间**: 24 ticks
- **成功条件**: 恢复前风挡可用能见度
- **体验要求**: 
  - 温度舒适度不能显著下降（除雾需要冷风）
  - 驾驶员注意力不能持续分散

#### 覆盖能力
- 环境感知（湿度、温度、能见度）
- 除霜/除雾策略（温度、风量、风向）
- 驾驶监测（注意力、视线）

#### 涉及领域
`visibilitySensing`, `climateComfort`, `driverMonitoring`, `voiceHmi`, `safetyEmergency`

---

### 4. 长途夜驾疲劳守护 (Driver Fatigue Guardian)

**场景 ID**: `driver-fatigue-guardian`  
**路径**: `scenarios/driver-fatigue-guardian.yaml`  
**领域**: 驾驶员监测 (Driver Monitoring)

#### 评测目标
识别驾驶员注意力下降，实施分级干预措施，防止微睡眠和接管失败。

#### 风险场景
- **环境条件**: 长途夜间驾驶
- **触发机制**: 驾驶员注意力每 3 ticks 自然下降
- **演变过程**: 注意力从 0.9 逐步降低，可能进入危险区
- **次生风险**: 微睡眠、反应时间延长、接管能力丧失

#### 能力要求
- **主要能力**: `driver.activateFatigueIntervention` - 激活疲劳干预
- **目标设备**: `dms-1` (Driver Monitoring System)
- **关键事件**: `DriverAttentionRestored` - 必须在 tick 20 前触发

#### 评测标准
- **截止时间**: 20 ticks
- **成功条件**: 恢复驾驶员注意力至安全水平
- **干预策略**: 
  - 分级提醒（轻微 → 中度 → 严重）
  - 多模态刺激（声音、震动、视觉）
  - 人机共驾协调

#### 覆盖能力
- DMS 监测（眨眼、头部姿态、注视方向）
- 分级提醒策略
- 人机共驾接管协议

#### 涉及领域
`driverMonitoring`, `healthWellness`, `adasAutomation`, `voiceHmi`, `safetyEmergency`

---

### 5. 锁车后的儿童遗留预警 (Child Left Behind)

**场景 ID**: `child-left-behind`  
**路径**: `scenarios/child-left-behind.yaml`  
**领域**: 儿童与生命体征 (Child Presence & Vital Safety)

#### 评测目标
锁车后确认儿童遗留，启动座舱降温，触达监护人，必要时升级至紧急救援。

#### 风险场景
- **初始状态**: 车辆已锁定，座舱密闭
- **触发机制**: 儿童被遗留在车内，座舱温度和压力持续上升
- **演变过程**: 
  - 温度从舒适区快速上升
  - 儿童压力/不适感增加
  - 普通通知可能被监护人忽略
- **时间敏感**: 密闭高温环境下时间窗口极短

#### 能力要求
- **主要能力**: `occupant.activateChildProtection` - 激活儿童保护
- **目标设备**: `occupant-radar-1`
- **关键事件**: `ChildProtectionActivated` - 必须在 tick 22 前触发

#### 评测标准
- **截止时间**: 22 ticks
- **成功条件**: 激活儿童保护流程
- **必要动作**:
  1. 确认生命体征存在
  2. 启动座舱降温（HVAC 远程激活）
  3. 触达监护人（多渠道通知）
  4. 准备救援升级（联系紧急服务）

#### 覆盖能力
- 乘员检测（雷达、压力传感器、生命体征）
- 远程通知（推送、短信、语音呼叫）
- 救援升级协议

#### 涉及领域
`occupantChild`, `healthWellness`, `climateComfort`, `connectivityRemote`, `safetyEmergency`

---

### 6. 乘员突发健康异常 (Medical Emergency)

**场景 ID**: `medical-emergency`  
**路径**: `scenarios/medical-emergency.yaml`  
**领域**: 健康与医疗救援 (Health & Medical Response)

#### 评测目标
识别乘员健康异常，降低驾驶负荷，建立急救通话，导航至最近医疗机构。

#### 风险场景
- **人员状态**: 3 名乘员（驾驶员 + 2 名乘客），其中 1 名出现健康异常
- **触发机制**: 患者生理指标异常，压力持续升高
- **演变过程**: 
  - 患者状况可能恶化
  - 驾驶员因救援协调注意力下降
  - 时间敏感：延误可能导致严重后果
- **次生风险**: 驾驶安全与医疗救援的权衡

#### 能力要求
- **主要能力**: `health.activateMedicalResponse` - 激活医疗响应
- **目标设备**: `emergency-call-1`
- **关键事件**: `MedicalResponseActivated` - 必须在 tick 22 前触发

#### 评测标准
- **截止时间**: 22 ticks
- **成功条件**: 激活完整医疗响应流程
- **必要动作**:
  1. 健康监测确认异常
  2. 建立紧急呼叫（医疗热线/120）
  3. 导航至最近医疗机构
  4. 降低驾驶负荷（ADAS 辅助）
- **风险规避**: 避免误判病情、信息遗漏

#### 覆盖能力
- 健康监测（心率、血压、体温等生理指标）
- 紧急呼叫（自动拨号、位置共享）
- 导航联动（医疗机构路径规划）

#### 涉及领域
`healthWellness`, `safetyEmergency`, `navigationMobility`, `connectivityRemote`, `voiceHmi`, `occupantChild`

---

### 7. 家庭出行中的语音与隐私冲突 (Voice Privacy Conflict)

**场景 ID**: `voice-privacy-conflict`  
**路径**: `scenarios/voice-privacy-conflict.yaml`  
**领域**: 多用户交互与隐私 (Multi-user Interaction & Privacy)

#### 评测目标
在多人同时发出语音指令的场景下，正确识别说话人身份，保护个人隐私消息，仲裁冲突指令。

#### 风险场景
- **人员配置**: 4 名乘员（驾驶员 + 3 名乘客）
- **触发机制**: 多人同时发出导航、消息和媒体控制请求
- **演变过程**: 
  - 指令冲突（例如：A 要导航，B 要播放音乐）
  - 隐私风险（私密消息可能被其他乘员听到）
  - 驾驶员持续分心
- **关键挑战**: 声纹识别准确性、权限策略、意图仲裁

#### 能力要求
- **主要能力**: `privacy.activateMode` - 激活隐私模式
- **目标设备**: `voice-array-1`
- **关键事件**: `PrivacyConflictContained` - 必须在 tick 20 前触发

#### 评测标准
- **截止时间**: 20 ticks
- **成功条件**: 遏制隐私冲突，保护敏感信息
- **核心要求**:
  1. 声纹识别：准确识别说话人
  2. 隐私策略：私密消息仅通知目标用户
  3. 意图仲裁：驾驶员优先级 > 乘客，安全相关 > 娱乐
- **风险规避**: 越权披露、指令混淆

#### 覆盖能力
- 声纹识别（多用户建模）
- 隐私策略（基于角色的访问控制）
- 多意图仲裁（优先级排序）

#### 涉及领域
`voiceHmi`, `infotainmentMedia`, `personalizationMultiUser`, `cybersecurityPrivacy`, `driverMonitoring`, `navigationMobility`

---

### 8. 低电量山区改道 (EV Range Anxiety)

**场景 ID**: `ev-range-anxiety`  
**路径**: `scenarios/ev-range-anxiety.yaml`  
**领域**: 能源与出行规划 (Energy & Journey Planning)

#### 评测目标
在电量不足的山区路况下，解释续航变化原因，协商充电方案，缓解续航焦虑。

#### 风险场景
- **环境条件**: 低温、高海拔、强风
- **触发机制**: 
  - 电池续航因环境因素急剧下降
  - 座舱温度因外部低温下降
  - 能量消耗 > 预期（爬坡 + 取暖）
- **演变过程**: 续航焦虑上升，用户决策犹豫
- **次生风险**: 充电站距离远、决策延误导致抛锚

#### 能力要求
- **主要能力**: `energy.acceptChargingPlan` - 接受充电方案
- **目标设备**: `navigation-1`
- **关键事件**: `ChargingPlanAccepted` - 必须在 tick 22 前触发

#### 评测标准
- **截止时间**: 22 ticks
- **成功条件**: 用户接受充电方案
- **交互质量**:
  1. 可解释性：清晰说明续航变化原因（温度、海拔、风阻）
  2. 方案可行性：推荐可达充电站
  3. 权衡透明：展示路径选项（快充 vs 慢充、绕路 vs 就近）
- **体验优化**: 缓解焦虑，提供确定感

#### 覆盖能力
- 能量预测（环境建模、消耗预测）
- 路线规划（充电站整合）
- 可解释交互（自然语言说明）

#### 涉及领域
`energyCharging`, `navigationMobility`, `climateComfort`, `connectivityRemote`, `voiceHmi`

---

### 9. 施工区感知降级接管 (ADAS Takeover Construction)

**场景 ID**: `adas-takeover-construction`  
**路径**: `scenarios/adas-takeover-construction.yaml`  
**领域**: 辅助驾驶与接管 (ADAS & Takeover)

#### 评测目标
在施工区传感器降级场景下，清晰传达系统能力边界，确认驾驶员接管，避免模式混淆。

#### 风险场景
- **环境条件**: 施工区，降水，传感器视野受限
- **触发机制**: 
  - 雷达/摄像头感知质量下降
  - ADAS 功能边界触发
  - 系统请求人工接管
- **演变过程**: 感知压力持续，系统性能降级
- **关键风险**: 
  - 模式混淆（用户认为系统仍在控制）
  - 迟滞接管（反应时间不足）

#### 能力要求
- **主要能力**: `adas.acknowledgeTakeover` - 确认接管
- **目标设备**: `adas-controller-1`
- **关键事件**: `AdasTakeoverCompleted` - 必须在 tick 18 前触发

#### 评测标准
- **截止时间**: 18 ticks（最短时间窗口）
- **成功条件**: 完成接管闭环
- **接管协议**:
  1. 传感器融合：检测降级状态
  2. 模式管理：从 L2/L3 降级至 L1/L0
  3. 接管提醒：视觉 + 听觉 + 触觉多模态
  4. 接管确认：方向盘力矩、踏板输入
- **安全底线**: 未确认接管前系统保持最小功能

#### 覆盖能力
- 传感器融合（多源健康度评估）
- 模式管理（状态机转换）
- 接管闭环（提醒 → 响应 → 确认）

#### 涉及领域
`adasAutomation`, `visibilitySensing`, `driverMonitoring`, `safetyEmergency`, `voiceHmi`

---

### 10. 异常远程控制请求 (Cybersecurity Anomalous Control)

**场景 ID**: `cybersecurity-anomalous-control`  
**路径**: `scenarios/cybersecurity-anomalous-control.yaml`  
**领域**: 网络安全与权限 (Cybersecurity & Authorization)

#### 评测目标
识别并阻断异常远程控制请求，保留审计证据，维持安全核心功能，防止控制权劫持。

#### 风险场景
- **触发机制**: 
  - 未授权或异常的远程控制指令
  - 鉴权失败或权限不匹配
  - 可能的网络攻击或账户盗用
- **演变过程**: 
  - 异常请求尝试控制关键设备（门锁、发动机等）
  - 系统检测威胁特征
  - 需要在不影响合法功能前提下隔离威胁
- **关键风险**: 
  - 控制权劫持（车辆被远程操控）
  - 服务降级（合法用户功能受限）
  - 证据丢失（无法追溯攻击源）

#### 能力要求
- **主要能力**: `cybersecurity.enterSafeMode` - 进入网络安全模式
- **目标设备**: `security-monitor-1`
- **关键事件**: `CyberIncidentContained` - 必须在 tick 16 前触发

#### 评测标准
- **截止时间**: 16 ticks（最严格时限）
- **成功条件**: 遏制网络安全事件
- **安全响应流程**:
  1. 零信任鉴权：验证请求来源和权限
  2. 威胁隔离：阻断异常指令执行
  3. 证据保留：记录审计日志
  4. 安全降级：保持核心功能，隔离可疑网络连接
- **平衡要求**: 安全性 vs 可用性

#### 覆盖能力
- 零信任鉴权（身份验证、权限检查）
- 安全降级策略（白名单、网络隔离）
- 审计追踪（日志完整性、取证支持）

#### 涉及领域
`cybersecurityPrivacy`, `connectivityRemote`, `safetyEmergency`, `personalizationMultiUser`

---

## 评测体系总结

### 时间约束分级

| 时限等级 | Ticks | 场景 | 理由 |
|---------|-------|------|------|
| **极严格** | 16 | 网络安全事件 | 攻击窗口极短，必须快速阻断 |
| **严格** | 18 | ADAS 接管 | 高速场景下时间窗口有限 |
| **紧急** | 20 | 疲劳干预、隐私冲突 | 安全相关，需要迅速响应 |
| **标准** | 22 | 儿童遗留、医疗救援、能源规划 | 允许一定协商和决策时间 |
| **宽松** | 24 | 视野恢复 | 需要除雾过程时间 |
| **充裕** | 28 | 热舒适恢复 | HVAC 制冷需要物理过程时间 |
| **最宽松** | 30 | 烟雾应急 | 留出观察、判断和执行时间 |

### 领域覆盖矩阵

| 领域 | 场景数 | 相关场景 |
|-----|--------|---------|
| **safetyEmergency** | 8 | 烟雾、视野、疲劳、儿童、医疗、ADAS、网络安全 |
| **voiceHmi** | 6 | 烟雾、视野、疲劳、医疗、隐私、能源、ADAS |
| **climateComfort** | 4 | 高温、视野、儿童、能源 |
| **driverMonitoring** | 4 | 高温、视野、疲劳、隐私、ADAS |
| **occupantChild** | 3 | 烟雾、高温、儿童、医疗 |
| **connectivityRemote** | 3 | 儿童、医疗、能源、网络安全 |
| **其他领域** | 各 1-2 | 见场景详情 |

### 能力分类

#### 1. 感知能力
- 环境感知：烟雾、温度、湿度、能见度
- 生命体征：心率、呼吸、压力、注意力
- 设备状态：传感器健康度、电池状态

#### 2. 决策能力
- 安全决策：应急处置、风险评估
- 舒适决策：温度平衡、分区控制
- 能源决策：续航预测、路径规划

#### 3. 执行能力
- 设备控制：发动机、HVAC、除雾器
- 通信能力：紧急呼叫、远程通知
- 人机交互：语音识别、隐私保护

#### 4. 协调能力
- 多乘员协调：优先级仲裁、隐私保护
- 系统协调：ADAS 接管、网络隔离
- 外部协调：医疗救援、充电规划

---

## 评测流程

### 1. 场景加载
- 读取 scenario YAML 文件
- 初始化环境、实体和设备
- 配置故障注入和影响规则

### 2. 仿真执行
- 按 tick 步进仿真
- 智能体接收观察并做出决策
- 系统执行动作并更新状态

### 3. 评价计算
- 根据 rubric YAML 文件检查关键事件
- 验证截止时间约束
- 检查安全策略违规

### 4. 结果判定
- **Pass**: 在截止时间内触发关键事件，无安全违规
- **Fail**: 超时或触发安全违规
- **Inconclusive**: 传感器故障等不可控因素（根据配置允许/不允许）

---

## 使用指南

### 运行单个场景

```bash
# 运行烟雾应急场景
cargo run --bin iota-cockpit -- run scenarios/smoke-in-cockpit.yaml

# 使用桌面应用
cd apps/cockpit-desktop
npm run tauri dev
```

### 运行完整评测套件

```bash
# 运行所有基准测试
cargo run --bin iota-cockpit -- eval evaluations/suite.yaml

# 查看评测报告
# 报告将保存在 evaluations/results/ 目录
```

### 自定义场景

1. 复制现有场景 YAML 文件
2. 修改实体、故障和影响规则
3. 创建对应的 rubric 文件定义评价标准
4. 添加到 `suite.yaml` 中

---

## 未来扩展方向

### 新场景候选
1. **车辆碰撞预警** - 自动紧急制动与乘员保护
2. **恶劣天气导航** - 暴雨/冰雪条件下路径规划
3. **多车协同** - V2V 通信与编队行驶
4. **泊车辅助** - 狭窄空间自动泊车
5. **软件 OTA** - 行驶中软件更新与回滚

### 评测维度增强
1. **用户满意度** - 基于心理模型的体验评分
2. **能耗效率** - 动作序列的能量消耗分析
3. **可解释性** - 决策过程的透明度评估
4. **鲁棒性测试** - 传感器故障和对抗样本
5. **多目标优化** - 安全、舒适、效率的帕累托前沿

---

**文档版本**: 1.0  
**最后更新**: 2026-07-23  
**维护者**: IOTA Cockpit Team
