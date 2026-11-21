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

插件改动还需构建实际 Wasm，运行通用 ABI 校验与各插件的专用检查器；
完整命令见 [CI 配置](.github/workflows/ci.yml)。界面改动需实窗验证，结束后关闭本次窗口与测试进程。
纯文档改动检查格式、链接和内容一致性；只报告实际完成的验证。

冻结现有 ITA 对照用例及断言。用户依赖的 legacy 行为继续作为 logisim-rs 回归；仅记录参考程序偶然行为的用例标为历史证据，后续单独审查归档或移除。新功能按项目规格验证。

## 无界面性能基线

在仓库根目录执行 `cargo build -p logisim-bench --release --locked --offline`，再执行 `target/release/logisim-bench single-cycle --runs 20`。负载可选 `wide`（独立 32 位加法器，每步改变一路）、`deep`（串联 NOT）、`single-cycle`（课程 CPU）或 `plugin`（课程寄存器文件）。用同一逻辑核顺序运行，每个负载至少三个独立进程；保存每次 stdout JSON、提交、工具链、CPU 和模块哈希。

每个进程记录一次从载入到首次收敛的 `cold_ms`，含模块载入、同步和 stable 计划编译；`plugin_instance_ms` 是另取一个已载入模块的实例化样本。每轮从复位开始；预热 3 轮后计时。两条时钟轨迹分别为 128 和 512 个 tick，组合负载每轮施加 512 次输入。寄存器文件每个 tick 先收敛新写入数据，再推进时钟并收敛。每次收敛及完整迭代分别计算最近秩 p50/p95；时钟负载另按奇偶 tick 分组，以区分两个边沿的开销。吞吐按有效迭代总数除以累计执行时间。时钟负载看 `ticks_per_s`，组合负载看 `iterations_per_s`。状态读取、四态轨迹校验、复位和 JSON 输出均在热计时区间外。步数、tick 数、插件读出/采样调用按单轮报告。跨进程吞吐极差除以中位数的目标为 ≤5%，基准耗时不作 CI 断言。

仅基准程序默认启用 `plugin-metrics`。用 `cargo run -p logisim-bench --release --no-default-features --locked --offline -- plugin --runs 20` 复核无埋点吞吐。桌面和 Agent 默认构建均关闭埋点。比较候选时保留两个二进制，在同一核心交替运行，分别比较完整 tick 吞吐及 outputs、transition 的 p95。

JSON 的 `plugin_phases` 分为 cold、reset 和 hot。reset 累计计时轮次开始前的初始化与输入复位。hot 的 `plugin_schedule_ns` 包含 outputs、transition 和 commit 调度，`guest_ns` 是其中 guest 分配缓冲及分派调用的墙钟时间。每种操作各自报告时延、信号槽数、两种传输的调用数、实际线上字节和有效状态字节。帧状态复制时间是 encode/decode 的子项；冷态 load_dependency、instantiate_stable 也含子阶段。缺席的 span 表示零次。只相加同一层的非重叠区间。

阶段 1 的课程寄存器文件对照使用同一 Wasm，保持每轮 512 tick、两次 settle/tick，以及四态摘要 `a2689cffb70ff825`。课程 CPU 对照为 128 tick，摘要 `73171884c0465aad`。宿主候选须达到插件吞吐提升 10% 或 outputs/transition 全调用 p95 降低 20%，且课程 CPU 稳定退化不超过 5%。在 i5-12600KF 上尝试合并同一输出帧的 UTF-8 文本校验，寄存器文件无埋点吞吐由 3,074 变为 3,072 tick/s，已撤回。

桌面快照构造可独立运行 `cargo test --release --locked --offline -p logisim-rs single_cycle_snapshot_cost -- --ignored --nocapture`；它使用真实视图适配器，不启动窗口。

[阶段 5 单线程热路径记录](apps/logisim-bench/comparisons/evaluator-hotpath.md)列出了 stable 与事件路径的热点、固定核交替测量方法，以及按负载区分的冷热结果。

### 复现 Agent 多会话测量

Linux 上先用 `cargo build --release --locked --offline -p logisim-agent` 构建，再运行 `python3 apps/logisim-bench/comparisons/agent-sessions.py`。脚本用课程 CPU 工程副本，以单连接运行 1、2、4 个 MCP 会话，各重复五次。每个会话交替发送 32 对 tick/state 请求，每次等待完整响应。工程打开与含插件加载的仿真初始化单独计时。检查两个 MCP 响应表示，并用顺序 CLI 的完整轨迹校验摘要，校验在热计时区间结束后进行。

可用 `--agent-cpus` 指定收发线程核集、`--worker-cpus` 为四个会话各指定一个逻辑核，并用 `taskset` 将客户端绑定到另一物理核。JSON 含每轮吞吐、各会话 p50/p95、服务进程峰值 RSS 和 worker 调度时间。汇总按五轮取中位数，尾延迟取每轮最慢会话的 p95。

### 复现 Wasm 引擎对照

默认桌面、Agent 与基准使用 wasmi。阶段 2 的[引擎契约记录](apps/logisim-bench/comparisons/engine-contract.md)给出两个 release 构建、固定核心、fuel 矩阵和冷启动复测命令。桌面与基准的双引擎构建使用 `wasmtime-experimental`；Agent 即使启用同一 feature 也固定运行 wasmi。基准可在负载前加 `--wasm-engine wasmi` 或 `--wasm-engine wasmtime`。只有研究用的 `compare-wasmtime` 增加私有磁盘缓存实验。阶段 1 的[原始对照补丁](apps/logisim-bench/comparisons/wasmtime-36.patch)对应历史提交 `cc5cee9`。

## 修改与提交

按职责在现有 crate 中扩展，避免课程电路特例。回归覆盖受影响的四态、实例隔离、编辑历史、文件往返与失败路径。
UI 文案集中于 `crates/logisim-ui/locales/`，使用 `tr` / `tf!`，控件 ID 不依赖翻译。

测试由所属模块的 `#[cfg(test)] mod tests;` 加载：小套件放 `tests.rs`，大套件按功能拆到
`tests/`，共享辅助代码放 `support`。跨模块流程放 crate 根测试入口；不为测试扩大生产 API，
不从其他用例文件导入辅助函数。

Git 采用 `.gitignore` 白名单，仅包含：

- 必要的 Rust 源码、Cargo/工具链文件、LICENSE 和 Git/构建配置。
- `crates/logisim-ui/locales/` 与自带插件 `locales/` 中编译必需的词条。
- 根目录 `README.md`、`README.zh-CN.md`、`CONTRIBUTING.md`、`AGENTS.md` 及 `assets/` 中的 README 截图。
- `examples/{course,game,agent}/*.circ`、`examples/game/*.{asm,hex}`、`examples/agent/{README.md,build_showcase.py}` 和自带插件的 `plugins/*/*.wasm`。

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
v0.1.0 额外保留 Intel macOS；v0.2.0 起不再提供 Intel macOS。各包包含桌面程序、无界面 Agent、电路示例、同次构建并校验的 Wasm、中英文 README、README 截图和 LICENSE。
普通分支构建不发布 Release；既有标签与资产不强制覆盖，修复先提交到版本分支。
