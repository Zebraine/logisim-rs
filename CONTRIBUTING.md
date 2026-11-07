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

Linux 构建需安装 `pkg-config` 和 Fontconfig 开发包（Debian/Ubuntu：`libfontconfig1-dev`）。
运行中文界面需系统中文字体，例如 Noto Sans CJK SC；字体文件不进入 Git。

```sh
cargo fmt --all -- --check
cargo fetch --locked
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
git diff --check
```

首次检查前安装 Rust 工具链并下载锁定依赖。测试覆盖布局、画布坐标、连接几何、
项目级编辑历史、接口连线、外观控制点/凹多边形/端口与锚/字体/封装往返、
递归保护、实例隔离/嵌套/多输出、
组合求值/逐位总线连接/扩展/网络错误/传播步进、时序边沿/异步控制/使能与片选/同沿采样/实例状态与时钟节奏、计数器溢出/新旧控制/加载掩码、四态移位/并行加载/位宽状态保持、文件往返/遗漏报告/磁盘失败保护及真实 egui 输入路径；
同时覆盖算术四态/进位借位/乘除辅助总线与32位边界/取负/比较与移位模式/层级进位、
多输入位计数及不确定结果前缀/四种位查找模式/输入数量编辑与连线保持，
以及 Plexers 选择/使能/三态/编码级联、分组取数/补零、七段译码双输入形式/控制优先级/输出保持与复位/实例隔离、端口角色保持及文件边界；语言切换、格式占位符、用户文本隔离、偏好持久化与失败重试。
界面改动还需真实启动窗口检查。每步实现前先运行参考程序并截图，行为依据遵循
[AGENTS.md](AGENTS.md) 中的 Cleanroom 约定。

新界面文案在 `crates/logisim-ui/src/i18n.rs` 中补齐词条；静态文案使用 `tr`，
动态文案使用 `tf!` 并将用户内容作为独立参数。格式参数数量在编译时检查。
语言属于显示状态，不能触发文档修改或仿真重置；库与窗口等可折叠/持久控件使用稳定 ID。
