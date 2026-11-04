# 开发与 Git 工作流

## 提交范围

每一步实现完成并验证后，按以下范围提交并推送到 origin，再进入下一步：

- `apps/`、`crates/` 中必要的 Rust 源码、包 manifest 和构建脚本。
- `Cargo.toml`、`Cargo.lock`、工具链及必要的 Cargo、格式和 lint 配置。
- `LICENSE`、Git 配置及 `.github/workflows/ci.yml` 构建检查配置。
- 根目录 `AGENTS.md`、`CONTRIBUTING.md`、`README.md`。

设计/进度文档、规格、实验、fixtures、resources、缓存及构建产物仅保留在本地，
不进入提交，也不能成为构建或 CI 的必需输入。新增必要源码或构建资产时，
按实际路径扩展 `.gitignore` 白名单，不整体放行资料目录。

提交前核对 `git diff --check`、`git diff --cached --check` 和
`git diff --cached --stat`。忽略规则不影响已跟踪文件；取消跟踪时保留本地内容。
是否暂存、提交或调整历史，遵循当次用户指令。

## 本地检查

```sh
cargo fmt --all -- --check
cargo fetch --locked
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
git diff --check
```

首次检查前安装 Rust 工具链并下载锁定依赖。测试覆盖布局、画布坐标变换与输入隔离；
界面改动还需真实启动窗口检查。每步实现前先运行参考程序并截图，行为依据遵循
[AGENTS.md](AGENTS.md) 中的 Cleanroom 约定。
