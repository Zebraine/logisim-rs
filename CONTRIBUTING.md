# 参与开发

行为观察与架构约束见 [AGENTS.md](AGENTS.md)。详细操作、元件规则、接口和验收记录保存在本地 `docs/`。

## 环境与检查

使用 `rust-toolchain.toml` 固定的工具链及 `Cargo.lock`。Linux 需 `pkg-config` 和 Fontconfig 开发包
（Debian/Ubuntu：`libfontconfig1-dev`）；中文界面使用系统字体。

Rust 代码改动执行以下检查：

```sh
cargo fmt --all -- --check
cargo fetch --locked
cargo check --workspace --all-targets --all-features --locked --offline
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo test --workspace --all-features --locked --offline
git diff --check
```

插件改动还需构建实际 Wasm，运行 `logisim-plugin-check` 和 `course-plugin-check`；
完整命令见 [CI 配置](.github/workflows/ci.yml)。界面改动需实窗验证，结束后关闭本次窗口与测试进程。
纯文档改动检查格式、链接和内容一致性；只报告实际完成的验证。

## 修改与提交

按职责在现有 crate 中扩展，避免课程电路特例。回归覆盖受影响的四态、实例隔离、编辑历史、文件往返与失败路径。
UI 文案集中于 `crates/logisim-ui/src/i18n.rs`，使用 `tr` / `tf!`，控件 ID 不依赖翻译。

测试由所属模块的 `#[cfg(test)] mod tests;` 加载：小套件放 `tests.rs`，大套件按功能拆到
`tests/`，共享辅助代码放 `support`。跨模块流程放 crate 根测试入口；不为测试扩大生产 API，
不从其他用例文件导入辅助函数。

Git 采用 `.gitignore` 白名单，仅包含：

- 必要的 Rust 源码、Cargo/工具链文件、LICENSE 和 Git/构建配置。
- 根目录 `README.md`、`CONTRIBUTING.md`、`AGENTS.md`。
- `examples/course/*.circ` 及共用的 `course.wasm`。

`docs/`、`specs/`、`experiments/`、`fixtures/`、`resources/`、缓存及构建产物留在本地，不作为构建依赖。
新增必要文件同步调整白名单，不用 `git add -f`。示例使用相对插件路径；更新模块后重新校验定义匹配和示例。

按用户当次指令暂存与提交；默认每步验证后提交并推送 origin，再进入下一步。提交前检查：

```sh
git diff --check
git diff --cached --check
git diff --cached --stat
```

网络失败可以重试，不强制推送或覆盖远端改动。

## 发布

1. 更新 workspace 版本，推送 `release/vX.Y.Z` 分支并确认构建通过。
2. 对通过检查的同一提交创建并推送附注标签 `vX.Y.Z`，标签须与 workspace 版本一致。
3. 标签构建通过后自动创建 Release；说明保持简短，附平台包与 SHA-256 校验文件。

发布流程见 [release.yml](.github/workflows/release.yml)。提供 Linux/Windows x86-64 和 macOS Apple Silicon；
仅 v0.1.0 额外保留 Intel macOS。各包包含 CPU 示例、同次构建并校验的 Wasm、README 和 LICENSE。
普通分支构建不发布 Release；既有标签与资产不强制覆盖，修复先提交到版本分支。
