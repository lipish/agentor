# Agentor

[English](#english) | [中文](#中文)

---

## English

### What is Agentor?

**Agentor** (Agent + Automator) is a local Git auto-deploy tool written in Rust, designed for solo developers. The core concept is to automatically complete the entire cycle of "build → deploy → sync to GitHub" after each code commit.

### Features

- **Post-commit Hook**: Automatically triggers deployment after every `git commit`
- **Auto Build**: Execute customizable build commands (cargo, npm, make, etc.)
- **Auto Deploy**: Support both process-based and file-based deployment
- **GitHub Sync**: Automatically push code to remote repository after successful deployment
- **Rollback Support**: Keep multiple versions and rollback when deployment fails
- **Logging**: Track all operations with timestamps and results

### Installation

#### From crates.io

```bash
cargo install agentor
```

#### From Source

```bash
git clone https://github.com/lipish/agentor.git
cd agentor
cargo build --release
cargo install --path .
```

### Quick Start

1. **Initialize Agentor in your Git repository**

```bash
cd your-project
agentor init
```

This will:
- Install a post-commit hook in `.git/hooks/post-commit`
- Create a default `deploy.toml` configuration file
- Create `.agentor/` directory for version management

2. **Configure `deploy.toml`**

Edit the generated `deploy.toml` file to match your project:

```toml
[watch]
repo_path = "."
branch = "main"

[build]
command = "cargo build --release"

[deploy]
command = "systemctl restart app.service"
target_dir = "/opt/deploy"
artifacts = ["target/release/my-app"]

[sync]
enabled = true
remote = "origin"
branch = "main"

[rollback]
enabled = true
keep_versions = 3

[log]
file = "agentor.log"
level = "info"
```

3. **Make a commit and watch it deploy automatically**

```bash
git add .
git commit -m "Your changes"
# Agentor will automatically trigger: build → deploy → sync
```

### CLI Commands

- `agentor init` - Initialize agentor in current Git repository
- `agentor run` - Manually trigger deployment workflow
- `agentor rollback` - Rollback to previous version
- `agentor status` - Show deployment status and history
- `agentor log` - Show recent deployment logs

### Configuration

See `deploy.toml.example` for a complete configuration reference.

#### Build Configuration

```toml
[build]
command = "cargo build --release"
```

Supports any build command: `npm run build`, `go build`, `make`, etc.

#### Deploy Configuration

Two deployment methods are supported:

1. **Process Deployment**: Execute a command
```toml
[deploy]
command = "systemctl restart app.service"
```

2. **File Deployment**: Copy artifacts to target directory
```toml
[deploy]
target_dir = "/opt/deploy"
artifacts = ["target/release/my-app", "config.json"]
```

#### Sync Configuration

```toml
[sync]
enabled = true
remote = "origin"
branch = "main"
```

#### Rollback Configuration

```toml
[rollback]
enabled = true
keep_versions = 3
```

### Workflow

```
git commit
    → post-commit hook triggers
    → agentor run
    → [1] Read deploy.toml configuration
    → [2] Execute build command
    → [3] Backup current version
    → [4] Copy artifacts to target directory
    → [5] Execute deployment command
    → [6] git push to GitHub
    → [7] Log results
    → Complete ✅

If any step [2]-[5] fails:
    → Auto rollback to previous version
    → Log error
    → Stop workflow
```

### License

MIT

---

## 中文

### Agentor 是什么？

**Agentor**（Agent + Automator）是一个用 Rust 实现的本地 Git 自动部署工具，面向单人开发者。核心理念是：代码提交后自动完成"构建 → 部署 → 同步到 GitHub"的完整闭环。

### 核心功能

- **Post-commit Hook 触发**：每次 `git commit` 后自动触发部署
- **自动构建**：支持任意构建命令（cargo、npm、make 等）
- **自动部署**：支持进程部署和文件部署两种方式
- **同步到 GitHub**：部署成功后自动 push 代码到远端仓库
- **回滚支持**：保留多个版本，部署失败时自动回滚
- **日志记录**：记录所有操作的时间戳和结果

### 安装

#### 从 crates.io 安装

```bash
cargo install agentor
```

#### 从源码安装

```bash
git clone https://github.com/lipish/agentor.git
cd agentor
cargo build --release
cargo install --path .
```

### 快速开始

1. **在你的 Git 仓库中初始化 Agentor**

```bash
cd your-project
agentor init
```

这会：
- 在 `.git/hooks/post-commit` 中安装 post-commit hook
- 创建默认的 `deploy.toml` 配置文件
- 创建 `.agentor/` 目录用于版本管理

2. **配置 `deploy.toml`**

编辑生成的 `deploy.toml` 文件以匹配你的项目：

```toml
[watch]
repo_path = "."
branch = "main"

[build]
command = "cargo build --release"

[deploy]
command = "systemctl restart app.service"
target_dir = "/opt/deploy"
artifacts = ["target/release/my-app"]

[sync]
enabled = true
remote = "origin"
branch = "main"

[rollback]
enabled = true
keep_versions = 3

[log]
file = "agentor.log"
level = "info"
```

3. **提交代码，自动部署**

```bash
git add .
git commit -m "Your changes"
# Agentor 会自动触发：构建 → 部署 → 同步
```

### CLI 命令

- `agentor init` - 在当前 Git 仓库中初始化 agentor
- `agentor run` - 手动触发部署流程
- `agentor rollback` - 回滚到上一个版本
- `agentor status` - 查看部署状态和历史
- `agentor log` - 查看最近的部署日志

### 配置说明

详细配置请参考 `deploy.toml.example`。

#### 构建配置

```toml
[build]
command = "cargo build --release"
```

支持任意构建命令：`npm run build`、`go build`、`make` 等。

#### 部署配置

支持两种部署方式：

1. **进程部署**：执行命令
```toml
[deploy]
command = "systemctl restart app.service"
```

2. **文件部署**：复制构建产物到目标目录
```toml
[deploy]
target_dir = "/opt/deploy"
artifacts = ["target/release/my-app", "config.json"]
```

#### 同步配置

```toml
[sync]
enabled = true
remote = "origin"
branch = "main"
```

#### 回滚配置

```toml
[rollback]
enabled = true
keep_versions = 3
```

### 工作流程

```
git commit
    → post-commit hook 触发
    → agentor run
    → [1] 读取 deploy.toml 配置
    → [2] 执行构建命令
    → [3] 备份当前版本
    → [4] 复制构建产物到目标目录
    → [5] 执行部署命令
    → [6] git push 同步到 GitHub
    → [7] 记录日志
    → 完成 ✅

如果步骤 [2]-[5] 任一失败：
    → 自动回滚到上一个成功版本
    → 记录失败日志
    → 终止流程
```

### 许可证

MIT