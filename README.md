# logisim-rs

[![Rust 1.99.0](https://img.shields.io/badge/Rust-1.99.0-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Total downloads](https://img.shields.io/github/downloads/Zebraine/logisim-rs/total?logo=github)](https://github.com/Zebraine/logisim-rs/releases)

基于 Rust + egui 的桌面数字逻辑电路编辑与仿真工具。
以 Logisim-ITA v2.16.2.2 的实际运行行为为兼容目标，采用 Cleanroom 方式独立实现。

## 功能

- 电路编辑、子电路、自定义外观与封装设计。
- 从真值表或表达式生成组合电路；真值表最多 12 输入，表达式最多 32 输入，均支持 32 输出。
- 门电路、布线、算术、Plexers、时序、存储器、输入/输出及基础 TTL 元件。
- 四态与层级仿真，可选择稳定态或事件传播模式。
- 已实现内容的 `.circ` 读写、ROM/RAM 的 `v2.0 raw` 格式 `.hex` 导入。
- `.circ` 子电路库与 Wasm 元件插件，支持英语和简体中文。

## 下载与运行

从 [Releases](https://github.com/Zebraine/logisim-rs/releases) 下载对应平台包及 `.sha256` 校验文件，完整解压后运行。

| 平台 | 启动方式 |
| --- | --- |
| Linux x86-64 | `./logisim-rs`；基于 Ubuntu 24.04 构建，需要 Fontconfig 和图形驱动 |
| Windows x86-64 | `logisim-rs.exe` |
| macOS 15+ | 打开 `Logisim rs.app`；Apple Silicon，v0.1.0 另提供 Intel 包 |

Windows/macOS 包未签名或公证；目前实机界面验证限于 Linux X11。
中文显示依赖系统字体，可安装 Noto Sans CJK SC。

从源码运行需 Rust 1.99+，仓库工具链固定为 1.99.0。Linux 构建另需 `pkg-config` 和 Fontconfig 开发包。

```sh
cargo run -p logisim-rs --release --locked
# 启动时打开示例
cargo run -p logisim-rs --release --locked -- examples/course/redirect-pipeline.circ
```

## 快速上手

- 从左侧选择元件，在画布放置；选中后编辑属性，从端口或线段中间拖出连线。
- 使用 Poke（`P`）操作输入，`Ctrl+T` 推进一个 tick，`Ctrl+K` 开关自动时钟。
- 自动时钟默认 4 Hz，可在 Simulate 菜单设置 1–4096 Hz；一个 tick 不等于完整时钟周期。
- 在 File → Settings 切换语言与仿真模式；默认英语、稳定态。
- 在 Project 菜单添加子电路、编辑封装、加载 `.circ` 库或 Wasm 插件。
- Project → Analyze Circuit 分析无状态组合电路；Generate Circuit 从真值表或表达式生成新电路，可选仅使用 NAND 门。
- ROM/RAM 的内容编辑器提供 `.hex` 导入；ROM 内容随电路保存，RAM 内容复位后清零。

## CPU 示例

[`examples/course/`](examples/course/) 包含 8 类 CPU 和 2 个测试，共用同目录的 `course.wasm`，无需 Java。
移动示例时请同时复制插件；打开后使用 Poke 按下并松开“复位”或“总复位”，再启动自动时钟。

| 文件 | 电路 |
| --- | --- |
| [single-cycle.circ](examples/course/single-cycle.circ) | 单周期 |
| [ideal-pipeline.circ](examples/course/ideal-pipeline.circ) | 理想流水线 |
| [bubble-pipeline.circ](examples/course/bubble-pipeline.circ) | 气泡流水线 |
| [redirect-pipeline.circ](examples/course/redirect-pipeline.circ) | 重定向流水线 |
| [dynamic-branch-prediction.circ](examples/course/dynamic-branch-prediction.circ) | 动态分支预测 |
| [single-interrupt.circ](examples/course/single-interrupt.circ) | 单级中断 |
| [nested-interrupt.circ](examples/course/nested-interrupt.circ) | 多级中断 |
| [pipeline-interrupt.circ](examples/course/pipeline-interrupt.circ) | 流水多级中断 |
| [pipeline-interrupt-benchmark.circ](examples/course/pipeline-interrupt-benchmark.circ) | 流水中断基准 |
| [instruction-coverage.circ](examples/course/instruction-coverage.circ) | 指令覆盖 |

重定向流水线和动态分支预测已核对完整运行轨迹，分别为 2298、1766 周期，最终显示 `0000 0038`。
全部示例已通过导入、往返保存、插件加载和复位后 128 tick 检查；其余示例尚未完成整机验收。

## 兼容范围

仅保存已实现内容；导入遇到不支持的内容会报告省略并禁止覆盖原文件。
`.circ` 库以可编辑快照导入。插件路径优先相对电路目录解析；缺失时可补选，未修复的元件显示静态图形、属性报错并输出 E。

宿主不执行 Java/JAR。含 Wasm 元件的 `.circ` 使用本项目扩展，需 logisim-rs 运行并附带模块。
不承诺完整兼容参考程序的全部元件、传播延迟与反馈行为。

## 开发与许可

开发、检查和发布流程见 [CONTRIBUTING.md](CONTRIBUTING.md)，协作约定见 [AGENTS.md](AGENTS.md)。

原创代码采用 [MIT](LICENSE)。
