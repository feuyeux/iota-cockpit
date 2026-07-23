# IOTA Cockpit 开发指南

## 快速开始

### 启动开发环境

```bash
# 方式 1: 使用 run.sh（推荐）
./run.sh

# 方式 2: 使用 Makefile
make dev

# 方式 3: 使用 npm
npm run dev
```

所有三种方式都会：
1. 检查并安装必要的依赖
2. 释放端口 15342（如果被占用）
3. 启动 Vite 开发服务器
4. 启动 Tauri 桌面应用

### 清理构建后启动

```bash
./run.sh --clean
# 或
make dev-clean
```

### 仅启动 Web 开发服务器

```bash
make dev-web
# 或
cd apps/cockpit-desktop && npm run dev
```

---

## 命令参考

### Makefile 命令

运行 `make help` 查看所有可用命令：

```bash
# 开发
make dev              # 启动桌面应用
make dev-clean        # 清理后启动
make dev-web          # 仅启动 web 服务器

# 构建
make build            # Debug 构建
make build-release    # Release 构建

# 测试
make test             # 运行所有测试
make test-rust        # 仅 Rust 测试
make test-desktop     # 仅 Desktop 测试
make test-watch       # 监视模式
make test-all         # 包含场景验证

# 代码质量
make lint             # 运行所有 linter
make lint-fix         # 自动修复问题
make validate         # 验证所有场景

# 仿真器
make simulator        # 运行默认场景
make simulator-live   # 实时模式
make simulator-live-acp # 带 ACP 的实时模式

# 评估
make eval-suite       # 运行评估套件（debug）
make eval-suite-release # 运行评估套件（release）

# 实用工具
make clean            # 清理构建产物
make info             # 显示环境信息
make scenarios        # 列出所有场景
```

### npm 脚本

```bash
# Workspace 级别（项目根目录）
npm run dev                  # 启动开发环境
npm run dev:clean            # 清理后启动
npm run dev:web              # 仅 web 服务器
npm run build                # 构建所有
npm run build:release        # Release 构建
npm test                     # 运行所有测试
npm run test:rust            # Rust 测试
npm run test:desktop         # Desktop 测试
npm run lint                 # 运行 linter
npm run lint:fix             # 修复问题
npm run validate             # 验证场景
npm run eval:suite           # 评估套件
npm run prepare:sidecar      # 构建 sidecar 二进制

# Desktop 应用级别（apps/cockpit-desktop）
cd apps/cockpit-desktop
npm run dev                  # Vite 开发服务器
npm run build                # 构建前端
npm test                     # 运行测试
npm run test:watch           # 监视模式
npm run test:ui              # UI 测试界面
npm run test:coverage        # 测试覆盖率
npm run tauri:dev            # Tauri 开发模式
npm run tauri:build          # 构建 Tauri 应用
npm run tauri:prepare-sidecar # 构建 sidecar
```

### 独立脚本

#### ./run.sh
主要开发入口，处理端口释放、依赖安装和环境设置。

```bash
./run.sh           # 启动开发环境
./run.sh --clean   # 清理 Rust workspace 后启动
./run.sh --help    # 显示帮助
```

#### ./scripts/test.sh
全面的测试运行器，支持选择性测试。

```bash
./scripts/test.sh                  # 运行 Rust + Desktop 测试
./scripts/test.sh --rust-only      # 仅 Rust 测试
./scripts/test.sh --desktop-only   # 仅 Desktop 测试
./scripts/test.sh --with-scenarios # 包含场景验证
./scripts/test.sh --with-lint      # 包含 lint 检查
./scripts/test.sh --all            # 运行所有检查
./scripts/test.sh --verbose        # 详细输出
```

#### ./scripts/eval.sh
评估套件运行器。

```bash
./scripts/eval.sh                              # Debug 模式
./scripts/eval.sh --release                    # Release 模式
./scripts/eval.sh --suite evaluations/custom.yaml
./scripts/eval.sh --output target/reports
./scripts/eval.sh --baseline baseline.json
./scripts/eval.sh --min-pass-rate 0.8
```

---

## 开发工作流

### 第一次运行

```bash
# 1. 克隆项目后
git clone <repo>
cd iota-cockpit

# 2. 直接启动（自动处理依赖）
./run.sh
```

### 日常开发

```bash
# 启动开发环境
make dev

# 运行测试（修改代码后）
make test

# 提交前检查
make check  # 等同于 lint + test + validate
```

### 构建发布版本

```bash
# 构建 release 版本
make build-release

# 或使用 npm
npm run build:release
```

### 运行评估

```bash
# 快速评估（debug）
make eval-suite

# 完整评估（release）
make eval-suite-release

# 或使用脚本（更多选项）
./scripts/eval.sh --release --baseline previous.json
```

---

## 故障排除

### 端口 15342 被占用

`run.sh` 会自动处理，但如果需要手动释放：

```bash
# macOS/Linux
lsof -ti:15342 | xargs kill -9

# Windows (PowerShell)
Get-Process -Id (Get-NetTCPConnection -LocalPort 15342).OwningProcess | Stop-Process -Force
```

### Sidecar 二进制文件缺失

```bash
# 手动构建 sidecar
npm run prepare:sidecar

# 或
cd apps/cockpit-desktop
npm run tauri:prepare-sidecar
```

### 依赖问题

```bash
# 重新安装 Node 依赖
cd apps/cockpit-desktop
rm -rf node_modules
npm install

# 清理 Rust 构建
cargo clean
```

### 测试失败

```bash
# 详细输出
./scripts/test.sh --verbose

# 仅运行失败的测试
cargo test --workspace -- <test_name>
```

---

## 环境要求

### 必需

- **Rust**: >= 1.95.0
- **Node.js**: >= 18.0.0
- **npm**: >= 9.0.0
- **Cargo**: 随 Rust 安装

### 可选

- **Python 3**: 用于校准脚本
- **lsof**: 端口管理（Unix 系统通常自带）
- **PowerShell**: Windows 端口管理

### 检查环境

```bash
make info
```

输出示例：
```
📋 Environment Information

Rust:
rustc 1.95.0
cargo 1.95.0

Node.js:
v20.10.0
9.8.0

Project:
  Root: /path/to/iota-cockpit
  Workspaces: apps/cockpit-desktop
```

---

## 项目结构

```
iota-cockpit/
├── run.sh              # 主开发入口
├── Makefile            # 统一命令接口
├── package.json        # Workspace npm 脚本
├── Cargo.toml          # Rust workspace 配置
├── scripts/
│   ├── test.sh         # 测试运行器
│   └── eval.sh         # 评估运行器
├── apps/
│   └── cockpit-desktop/
│       ├── package.json          # Desktop 应用配置
│       ├── src/                  # React 源码
│       ├── src-tauri/            # Tauri Rust 代码
│       └── scripts/
│           └── prepare-sidecar.mjs
├── crates/             # Rust crates
├── scenarios/          # 仿真场景
└── evaluations/        # 评估配置
```

---

## 参考文档

- [README.md](README.md) - 项目概述和架构
- [docs/user-guide-zh.md](docs/user-guide-zh.md) - 用户指南
- [docs/scenario-evaluation-objectives.md](docs/scenario-evaluation-objectives.md) - 评估目标

---

## 常见任务

### 添加新场景

1. 在 `scenarios/` 创建 YAML 文件
2. 验证场景：`cargo run -p cockpit-simulator -- validate scenarios/new-scenario.yaml`
3. 运行场景：`cargo run -p cockpit-simulator -- run scenarios/new-scenario.yaml`

### 修改 UI

1. 编辑 `apps/cockpit-desktop/src/` 中的文件
2. 热重载会自动更新（如果 `run.sh` 正在运行）
3. 运行测试：`cd apps/cockpit-desktop && npm test`

### 修改仿真器

1. 编辑 `crates/` 中的 Rust 代码
2. 重新构建：`cargo build -p cockpit-simulator`
3. 运行测试：`cargo test -p cockpit-simulator`

### 调试

```bash
# Rust 日志
RUST_LOG=debug cargo run -p cockpit-simulator -- run scenarios/smoke-in-cockpit.yaml

# 前端开发工具
# 在浏览器中打开 http://127.0.0.1:15342
# 使用 Chrome/Edge DevTools

# Tauri DevTools
# 应用运行时按 F12
```
