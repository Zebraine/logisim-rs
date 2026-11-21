# logisim-rs

[English](README.md) | [简体中文](README.zh-CN.md)

[![Rust 1.99](https://img.shields.io/badge/Rust-1.99.0-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Total downloads](https://img.shields.io/github/downloads/Zebraine/logisim-rs/total?logo=github)](https://github.com/Zebraine/logisim-rs/releases)

基于 Rust + egui 的桌面数字逻辑电路编辑与仿真工具。
支持打开和编辑已实现子集内的 Logisim-ITA v2.16.2.2 旧工程。

<p align="center">
  <img src="assets/workbench-light.png" alt="浅色工作台：编辑流水线 CPU" width="49%">
  <img src="assets/workbench-dark.png" alt="深色工作台：实时信号颜色" width="49%">
</p>

## 可以做什么

- 编辑层次化电路：子电路、自定义外观、可编辑封装。
- 从真值表或表达式生成组合电路；真值表最多 12 输入，表达式最多 32 输入，均支持 32 输出。
- 使用门电路、布线、算术、Plexers、时序、存储器和输入/输出元件。
- 四态、层次化仿真，支持稳定态或事件传播模式，并实时显示导线值与信号颜色。
- 读写原生 `.lrs` 工程和已支持的 `.circ` 子集，导入 ROM/RAM 的 Logisim `v2.0 raw` `.hex` 文件。
- 将 `.circ` 库作为可编辑快照导入，并从 Wasm 插件加载元件。
- 英语、简体中文；浅色/深色主题，三种强调色，两种密度，可改绑的快捷键。
- 自动保存、崩溃恢复、可选的单代 `.bak` 保存前备份。
- 将当前画布视口导出为 PNG 图片。

## 开始使用

### 下载与运行

从 [Releases](https://github.com/Zebraine/logisim-rs/releases) 下载对应平台包和 `.sha256` 校验文件，校验后完整解压再运行。

| 平台 | 启动方式 |
| --- | --- |
| Linux x86-64 | `./logisim-rs`；基于 Ubuntu 24.04 构建，需要 Fontconfig 和可用的图形驱动 |
| Windows x86-64 | `logisim-rs.exe`；无界面 CLI/MCP 服务为 `logisim-agent.exe` |
| macOS 15+ | 打开 `Logisim rs.app`（Apple Silicon）；无界面 CLI/MCP 服务为 `./logisim-agent` |
| 所有平台包 | 包含无界面 `logisim-agent`；Linux 使用 `./logisim-agent`。 |

Windows/macOS 包未签名或公证。本版本的界面实机验证在 Linux X11 上完成；其他平台包由同一发布流程构建，但尚未在对应系统上人工验证界面。

本版本的新工程和另存为默认使用二进制原生 `.lrs`；已有 `.circ` 仍可读取。v0.4.0 是最后一个默认使用 `.circ` 的版本。

中文显示依赖系统字体；如有缺字，可安装 Noto Sans CJK SC 等 CJK 字体。

### 第一个电路

1. 用 **File → New** 新建电路，或打开原生[计数器示例](examples/agent/counter.lrs)。
2. 从左侧目录选择元件，放到画布上，在检查器中编辑属性。
3. 从元件端口或导线中段拖出连线。
4. 选择 Poke 工具（`P`）操作输入、查看信号。
5. 在 Simulate 菜单使用 **单步** / `Ctrl+I`、**时钟步进** / `Ctrl+T` 或 **自动时钟** / `Ctrl+K` 推进时间。
6. 用 `Ctrl+S` 保存。标题栏会显示是否有未保存修改。

一个 tick 是仿真步进，不一定等于完整时钟周期。自动时钟默认 4 Hz，可在 Simulate 菜单调整。

常用起点：

- `V` 选择，`P` 交互，`W` 连线，`T` 文字，`U` 隧道，`R` 旋转。
- `Ctrl+Z` / `Ctrl+Shift+Z` 撤销、重做。
- `Ctrl+F` 按当前焦点搜索目录或设置。
- **File → Settings** 可切换语言、仿真模式、主题、密度、编辑器默认值、自动保存和恢复行为。
- **文件 → 打印…** 可预览当前画布，确认后将预览图发送至系统默认打印机。Linux 和 macOS 需要 `lp`，Windows 通过 PowerShell 调用已注册的 PNG 打印程序。发送期间可取消打印命令，超过 20 秒应用会终止该命令。已经进入系统打印队列的作业可在队列中取消。纸张、份数及其他页面选项沿用系统默认值。
- **设置 → 编辑器** 可为新元件标签设置字体（界面字体、等宽或衬线）、字号、粗体和斜体。已记住的放置模板保留手工设置的字体。画布使用系统衬线字体显示 Serif。找不到可用衬线字体时，使用比例字体并在启动日志中记录一次警告。
- 快捷键可在 **Settings → Shortcuts** 中改绑。

## 插件系统

logisim-rs 使用带版本号的 Wasm 插件 ABI 提供外部元件。插件可以定义图形、端口、属性、动作和仿真行为；桌面宿主负责加载、校验，并接入项目。

- 插件是独立的 `.wasm` 文件，不编译进桌面二进制。
- 项目将插件依赖记录为来源路径；相对路径优先从电路目录解析，绝对路径作为备用。
- 缺失依赖可在 **Project Options → Resolve missing…** 中补选。**Settings → Advanced → Plugin fallback directory** 可为本机模块增加一个可选的同名兜底目录。
- 使用前会校验插件路径和元件定义；缺失或不匹配的模块会明确保留为不可用状态，不会被静默替换。
- 宿主不执行 Java，也不读取 JAR 内部实现。

六个随附插件均使用 ABI 4，声明支持 wasmi 与 Wasmtime。模块路径、插件 ID、版本（1.0.0）及工具定义均与原版一致。包内 `plugins/abi3/` 另附六份原版模块，供旧宿主和需要原有计量行为的工程使用。

桌面默认使用 wasmi。默认 Cargo 构建和官方桌面包都包含 wasmi 与 Wasmtime；Wasmtime 代码编入桌面程序，但运行时默认仍是 wasmi。需要 only-wasmi 的开发构建时使用 `--no-default-features`。可在**设置 → 高级 → Wasm 插件引擎**选择 Wasmtime，确认并重启。**项目选项**会列出每个已加载依赖的请求引擎和实际引擎。ABI 1–3 始终使用 wasmi。ABI 4 声明不允许所选引擎时，宿主拒绝加载并报告原因。Agent 固定使用 wasmi。

ABI 4 每次请求的预算为所选引擎的 1,000 万个 fuel 单位。同一计算可能在一个引擎完成，在另一个引擎耗尽预算。稳定态执行故障会暂停仿真并丢弃待提交的 transition，Reset 重建实例。本次 Linux 实测中，空缓存首次编译课程模块约需半秒。

Linux 桌面将编译后的机器码存放在应用偏好文件旁的私有目录 `wasmtime-cache-v1/`，应使用可信本地文件系统。目录不可用或记录损坏时，桌面从源 Wasm 编译并向 stderr 报告原因。退出桌面后可删除目录以清空缓存。Windows 和 macOS 上的 Wasmtime 每次加载时从源模块编译。私有磁盘缓存目前依赖 Linux 的所有者与权限校验。

## 示例

仓库在 [`examples/course/`](examples/course/) 中提供可选的课程 CPU 示例。它们共用课程 Wasm 模块，定位是教学材料，而不是固定基准套件。

有代表性的起点：

- [`single-cycle.circ`](examples/course/single-cycle.circ) — 单周期 CPU。
- [`ideal-pipeline.circ`](examples/course/ideal-pipeline.circ) — 理想流水线 CPU。
- [`redirect-pipeline.circ`](examples/course/redirect-pipeline.circ) — 带重定向处理的流水线 CPU。
- [`dynamic-branch-prediction.circ`](examples/course/dynamic-branch-prediction.circ) — 动态分支预测。
- [`single-interrupt.circ`](examples/course/single-interrupt.circ) 与 [`nested-interrupt.circ`](examples/course/nested-interrupt.circ) — 中断变体。

Agent 示例以原生格式提供[计数器](examples/agent/counter.lrs)、[加法器](examples/agent/adder.lrs)和[插件寄存器文件](examples/agent/register-file.lrs)。每个示例随包保留对应的旧版 `.circ` 源文件。[示例指南](examples/agent/README.md)列出了输入、预期探针读数和显式导出命令。

示例用于演示已实现行为，并不是所有时序或异常细节的兼容性证明。移动示例时请保留 `examples/` 与 `plugins/` 的相对目录，或使用插件补选功能。

## 工程格式与迁移

Logisim-ITA 兼容范围已冻结，新功能以 logisim-rs 自身规格为准。`.circ` 作为旧工程迁移格式，已支持的子集继续读写。导入时报告未知或未支持的内容，并保护源文件。导入的库仍为可编辑快照，Wasm 元件需要对应的模块。外部编辑器重存工程时可能丢弃 `agent-id` 等扩展属性。

新工程首次保存和“另存为”默认使用原生 `.lrs` v1（二进制 CBOR）。打开旧 `.circ` 后，“保存”仍写回原 XML 文件。用“另存为”可切换当前文件。要另存为 XML，请选择“旧版 Logisim (*.circ)”并确认，文件扩展名须与所选类型一致。崩溃恢复沿用原文档格式。

开发阶段生成的 JSON `.lrs` 可用 `logisim-agent lrs migrate --input old.lrs --output new.lrs` 迁移到单独的二进制副本。要查看原生工程的 JSON 内容，可运行 `logisim-agent lrs inspect --input new.lrs --output project.json`。两个命令均要求输出文件尚未存在。

在“文件”菜单选择“转换为 .lrs…”或“导出 .circ…”可写出单独副本，并继续编辑原工程。只有完整导入的工程可以转换。导出 `.circ` 前会回读并核对文档，报告旧格式无法保留的属性。如需编辑副本，请打开新文件。原生文档为电路、元件和导线保存持久 ID，子电路实例与外观端口通过持久 ID 引用对象。导线端点保留画布坐标，Wasm 模块仍是外部依赖。

## 从源码构建

需要 Rust 1.99 或更新版本（仓库通过 `rust-toolchain.toml` 固定 1.99.0），以及 eframe/egui 所需的平台库。

Debian/Ubuntu 先安装 Linux 构建依赖：

```sh
sudo apt-get update
sudo apt-get install -y pkg-config libfontconfig1-dev
```

在工作区运行：

```sh
cargo run -p logisim-rs --release --locked
# 启动时打开示例
cargo run -p logisim-rs --release --locked -- examples/course/redirect-pipeline.circ
```

运行工作区检查：

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked --offline
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo test --workspace --all-features --locked --offline
```

开发、检查和发布流程见 [CONTRIBUTING.md](CONTRIBUTING.md)。插件构建、ABI 校验和发布打包见 [.github/workflows/ci.yml](.github/workflows/ci.yml) 与 [.github/workflows/release.yml](.github/workflows/release.yml)。

## 无界面 Agent

发布包已包含无界面 `logisim-agent`。也可用 `cargo build --release --locked -p logisim-agent` 从源码构建。生产环境建议将 MCP 客户端的命令设为 `logisim-agent`，参数设为 `--root /work/project mcp`。JSON CLI 也支持相同的根目录。

```sh
logisim-agent --root /work/project mcp
logisim-agent --root /work/project project open --path example.circ
logisim-agent --root /work/project settings get --config preferences.json
logisim-agent project catalog
logisim-agent settings catalog
logisim-agent settings get --config /path/to/preferences.json
logisim-agent settings patch --config /path/to/preferences.json --ops edits.json --dry-run
```

请选择仅由可信本机进程管理的现有目录。指定 `--root` 后，工程、配置和 CLI `--ops` 的相对路径均从该目录解析。Agent 在文件读写前校验绝对路径、符号链接和保存所用的锁与备份路径，越界返回 `path_outside_root`。省略该选项时，相对路径从工作目录解析，绝对路径不受此限制。工程中记录的插件依赖沿用工程自身的解析规则。

MCP 支持 `2024-11-05`、`2025-03-26`、`2025-06-18` 和 `2025-11-25`。握手时原样返回支持的版本，对其他版本返回 `2025-11-25`。调用 `tools/call` 后，客户端可读取 `result.isError` 判断执行结果。`structuredContent` 与 `content[0].text` 中的 JSON 携带相同的 CLI 响应，失败时保留错误码和详情。请求格式、未知工具与握手错误通过 JSON-RPC 顶层错误返回。单次 CLI 命令失败时输出顶层 `ok:false` 并以非零状态退出。

例如，`{"ops":[{"key":"appearance.theme","value":"dark"}]}` 修改 v17 便携偏好。可在同一 JSON 对象中将 `expected_sha256` 设为读取时返回的 `source_sha256`，要求原文件仍为该版本。确认预演结果后，用 `--apply` 写入。桌面端与 Agent 通过同目录的 `.lock` 文件协调原子更新，保留最近工程、窗口几何、面板布局和本机插件目录。

`plugins.wasm_engine` 保存桌面偏好（`wasmi` 或 `wasmtime`）。Agent 在所有构建中均使用 wasmi。`simulation_state` 的 `wasm_engine` 标出插件运行时，`engine` 表示电路传播引擎。设置响应也会返回 `agent_wasm_engine`。

读取电路时，启动 `logisim-agent cli`，在同一个进程内逐行发送 JSON。每个进程最多同时打开 64 个工程会话，达到上限时 `project_open` 返回 `session_limit`。使用 `project_close` 可释放名额。

MCP 按到达顺序执行同一工程会话的请求。独立会话可并行执行，单个电路的仿真保持单线程。请用 JSON-RPC ID 匹配响应。设置操作和 CLI 行协议仍按顺序执行。

```jsonl
{"tool":"project_open","arguments":{"path":"examples/agent/counter.lrs"}}
{"tool":"project_snapshot","arguments":{"session":"ID_FROM_OPEN"}}
{"tool":"simulation_open","arguments":{"session":"ID_FROM_OPEN"}}
{"tool":"simulation_state","arguments":{"session":"ID_FROM_OPEN"}}
{"tool":"project_close","arguments":{"session":"ID_FROM_OPEN"}}
```

后续请求使用打开工程时返回的 `data.session_id`。CLI 和 MCP 提供相同的 20 项操作。MCP 握手后可用 `tools/list` 查询工具名及输入 schema。工程快照覆盖全部电路定义，仿真状态包含根实例的信号、时钟与传播计数、诊断和插件状态。

### 控制仿真

首次打开时可选 `auto`（默认）、`stable` 或 `event`。Auto 和 Stable 优先使用编译传播，预检回退的原因记录在 `fallback_reason`。运行中发生故障会暂停。同一会话再次打开会返回 `created:false`、当前模式和状态。保存工程保留仿真，已应用且改变工程的补丁使仿真失效，关闭会话释放仿真。

在同一个 `logisim-agent cli` 进程中，可逐行发送命令参数数组。

```jsonl
["simulation","open","--session","ID_FROM_OPEN","--mode","event"]
["simulation","enable","--session","ID_FROM_OPEN","--enabled","false"]
["simulation","set-input","--session","ID_FROM_OPEN","--id","1","--value","1"]
["simulation","step","--session","ID_FROM_OPEN"]
["simulation","settle","--session","ID_FROM_OPEN"]
["simulation","tick","--session","ID_FROM_OPEN"]
```

从根电路快照取输入 ID。输入 Pin、Switch 和 DipSwitch 接受其控制位宽内的无符号数，三态输入 Pin 也支持 `--unknown`。启用时输入会传播至稳定，暂停时可用 `step` 推进一个事件批次、`settle` 完成传播，或用 `enable --enabled true` 恢复传播。网络稳定后，`tick` 推进一拍并完成传播。实例层级无时钟时返回 `no_clocks`，有待传播事件时返回 `not_settled`。时钟只由显式 tick 推进。Reset 恢复初始信号，重新收敛后显示的拍数与步数均为零。每个成功控制操作返回 `action`、`changed` 和操作后的 `state`。

根电路插件实例的动作 ID、标签和参数说明可从 `project_snapshot` 的 `tool.actions` 读取。打开仿真后，调用 `simulation_plugin_action` 时传入组件的 `id` 或 `agent_id`，两者选一。例如，课程 Register File 的 `write` 接受十进制寄存器地址和十六进制数据。

```jsonl
["simulation","plugin-action","--session","ID_FROM_OPEN","--agent-id","COMPONENT_AGENT_ID","--action","write","--argument","1 DEADBEEF"]
```

参数为按原样传递的 UTF-8 文本，最多 1024 字节，省略时取空字符串。没有参数说明的动作只接受空字符串。动作在 Agent 会话自己的仿真实例中执行，与已打开的桌面窗口相互独立。键盘绑定和 `InputEvent` 事件派发仅在交互式编辑器内可用。成功响应包含 `action:"plugin_action"`、`changed` 和完整仿真状态。即使输出引脚读数相同，插件内部状态发生变化也会计入 `changed`。暂停时动作仍会执行，待传播的信号计入 `pending_count`，可手动传播或恢复运行。动作保留当前 tick 和工程文件内容。目标、动作或参数校验失败时，仿真状态保持原值。插件执行故障返回 `plugin_error`，`error.details.state` 包含暂停后的状态，可用 `simulation_reset` 恢复。

### 编辑工程会话

向 `project_patch` 传入 `session`、明确的 `dry_run` 布尔值和有序的 `ops` 数组。预演会返回预计分配的 ID；会话状态相同时，应用同一批操作会得到相同的 ID。单项失败会撤销整批修改，`error.details.op_index` 从零开始标识失败项。提交编辑后，重新调用 `simulation_open` 即可观察新工程的初始仿真状态。

```jsonl
{"tool":"project_patch","arguments":{"session":"ID_FROM_OPEN","dry_run":true,"ops":[{"op":"update_component","circuit":1,"id":12,"properties":{"label":"A","facing":"west"}}]}}
{"tool":"project_patch","arguments":{"session":"ID_FROM_OPEN","dry_run":false,"ops":[{"op":"update_component","circuit":1,"id":12,"properties":{"label":"A","facing":"west"}}]}}
{"tool":"project_save","arguments":{"session":"ID_FROM_OPEN","expected_sha256":"SHA_FROM_OPEN_OR_LAST_SAVE","backup":true}}
```

在当前会话调用 `project_convert`，传入 `session` 和 `output`，即可写出独立的 `.lrs` 或 `.circ` 副本。CLI 行协议的等效命令为 `project convert --session ID --output board.lrs`。目标存在时须提供目标原始字节的 `expected_output_sha256`（CLI 参数为 `--expected-output-sha256`），默认保留一代备份。成功响应返回副本的 `path`、`format` 和 `sha256`。在 CLI 和 MCP 中，`project_save` 始终写回会话打开的路径与格式。打开的是 `.circ`，保存的仍是 XML。

请使用快照中的真实 ID。核对预演结果后，将同一补丁的 `dry_run` 改为 `false`，应用后再保存。`project_patch` 的输入 schema 列出电路、导线和元件操作，包括用于 Wasm 实例的 `add_plugin` 和 `update_plugin`。端口坐标使用画布坐标，颜色使用 RGBA，Splitter 映射使用从零开始的分支索引，断开位用 null。多个电路内有相同的元件或导线数字 ID 时须传 `circuit`。导线遵循编辑器的重布线和合并、分段规则。

打开工程前即可运行 `logisim-agent project catalog`，或无参数调用 MCP 工具 `project_catalog`。`data.kinds` 列出 Core 规范名、可用别名，以及 `addable`、`editable` 和 `requires_circuit_id` 标志。仅子电路实例需要目标电路 ID。插件条目标有 `requires_plugin_source:true`，其属性由选定模块和工具决定。

对于可静态添加的种类，元件快照的 `property_schema` 与目录条目的 `properties` 一致，给出枚举值、整数范围、RGBA 通道、允许分支为空的 Splitter 映射和可部分更新的字体对象。有字节长度限制的字符串通过 `x-maxUtf8Bytes` 给出 UTF-8 上限。最终元件还须满足字段间约束，例如 Counter 最大值须能用其位宽表示。快照保留 `patchable_properties`、`property_types` 以及同名字段中的当前值。添加和更新使用同一套属性。传入部分 `font` 字段只修改对应的字体属性。

| 可编辑元件 | 主要属性 |
| --- | --- |
| Pin、Gate、Parity、Probe、Tunnel、Splitter | 位宽、输入、进制或分支映射，以快照为准 |
| Constant、Ground、Power、Pull Resistor | 信号源位宽 `bits`、位宽内的无符号常量 `value`、电阻 `pull`（字符串 `0`、`1`、`X`） |
| Button、Switch、DipSwitch、Clock | 开关位宽、拨码数 `switch_count`、时钟高低电平持续 tick 数 `high_ticks` 和 `low_ticks`（1–65535）、标签位置 |
| Text、Bit Extender | 文本 `text`、字体、RGBA 颜色、`halign`（left/center/right）、`valign`（top/center/base/bottom）。扩展器 `in_width`、`out_width`、`type`（zero/one/sign/input） |
| Shifter、Adder、Comparator | `bits`、移位模式 `shift`（logical_left/logical_right/arithmetic_right/rotate_left/rotate_right）、有符号比较 `signed` |
| Multiplexer、Demultiplexer、Decoder | 选择位宽 `select_bits`（1–5）、`enable`、`top_right`、`disabled_zero`。复用器和解复用器支持数据位宽，解复用器和译码器支持 `three_state` |
| 减法器、乘法器、除法器、取负器、Bit Adder、Bit Finder | `bits`（1–32）、Bit Adder 输入数 `bit_inputs`（1–32）、Bit Finder 查找模式 `search`（lowest_one/highest_one/lowest_zero/highest_zero） |
| Priority Encoder、Bit Selector、7-Segment Display Decoder | 优先编码器支持 `select_bits`、`disabled_zero`、`no_input_zero`。位选择器支持 `bits`、`group_bits`（1–32）和 `top_right`。七段译码器支持 `multibit` |
| 受控缓冲器、受控反相器、晶体管、传输门 | 均支持位宽和朝向。受控门支持 `control_left`、`negate_control`，反相器另支持 `size`（narrow/wide）。晶体管和传输门支持 `gate_bottom_right`，晶体管另支持 `transistor_type`（p/n） |
| D、T、J-K、S-R 触发器与 D、S-R 锁存器 | `trigger` 对触发器支持 rising/falling，对锁存器支持 high/low。S-R 锁存器还可选 latch 表示无使能。均支持 `controls`（above_below/below_above/legacy）、`negate_controls`、`new_layout`。旧触发器布局要求 legacy 控制位置 |
| Register、Counter、Shift Register、Random Generator | `bits` 与 `trigger`（rising/falling，Register 另支持 high/low）。寄存器支持 `select_high`，计数器支持 `counter_max`、`counter_overflow`（wrap/stay/continue/load）和 `counter_old`，随机数发生器支持有符号 32 位 `random_seed` |
| LED、SevenSegment、HexDisplay、RGBLED、DotMatrix | 适用的 RGBA 颜色和极性、RGB 多位输入、点阵行列数（1–32）、`matrix_input`（column/row/select）、圆点和余辉时间 |
| ROM、RAM | 均支持 `bits`（1–32）、`address_bits`（2–24）、`select_high` 和标签属性。ROM 支持 `contents`。RAM 支持 `interface`（shared_sync/shared_async/separate）和 `simple`，其存储内容属于仿真实例 |
| PLA ROM | `inputs`、`terms`、`outputs`（各 1–32）、`input_plane`、`output_plane`、`select_high` 和标签属性 |
| Slider、Joystick | Slider 支持 `bits`（1–32）、`slider_reverse`、最终位宽内的无符号 `slider_value`、朝向、标签、字体及颜色。Joystick 支持 `facing` 和 `bits`（2–5） |
| Keyboard、TTY | 均支持 `bits`（7 或 16）和 `trigger`（rising/falling）。Keyboard 支持 `buffer_len`（1–256）。TTY 支持 `rows`（1–48）、`columns`（1–120）和 RGBA 前景色 `on_color`、背景色 `background` |

ROM 的 `contents` 整体替换镜像，最多接受 65,536 个地址严格递增的 `[address, word]` 边界。每个值持续到下一边界或容量末尾。例如 `[[2,7],[5,0]]` 将地址 2 至 4 设为 7，其余字为零。空数组清空镜像。仅调整尺寸时沿用 core 规则，丢弃新容量外的地址，并按新位宽屏蔽高位。已有 ROM 超过边界数量上限时，快照返回 `too_large`。

PLA 的两个平面均以项为行。输入单元取 `disconnected`、`direct` 或 `inverted`，输出单元取布尔值。提交的平面须符合最终尺寸。调整尺寸会保留重叠单元，并在 patch 结果中返回两个完整平面。

添加子电路实例时分别指定父电路和已有的目标定义，例如 `{"op":"add_component","circuit":1,"kind":"subcircuit","circuit_id":2,"position":[100,100]}`。目标定义可用 `circuit_id` 或 `circuit_agent_id` 指定，两者只传一个。父电路 `circuit` 使用数字 ID。快照中 `circuit_name` 和 `interface_ports` 给出目标名称与 Pin 绑定。实例可更新标签、字体、朝向和位置。修改目标 Pin 会同步刷新实例接口。递归引用和删除仍被引用的电路会被拒绝。原有 66 个种类各有静态属性定义，插件实例通过 `add_plugin` 添加。

时序元件补丁编辑文档属性，运行状态通过仿真工具查看。若缩小计数器位宽后原上限超出范围，请在同一操作中指定新的 `counter_max`。

使用 CLI 参数时，可将 `{"ops":[...],"expected_sha256":"..."}` 放入编辑文件，哈希可省略。已启动的 `logisim-agent cli` 或 `project open` 进程也接受逐行输入的参数数组。

```jsonl
["project","patch","--session","ID_FROM_OPEN","--ops","edits.json","--dry-run"]
["project","patch","--session","ID_FROM_OPEN","--ops","edits.json","--apply"]
["project","save","--session","ID_FROM_OPEN","--expected-sha256","SHA_FROM_OPEN_OR_LAST_SAVE"]
```

显式保存才会写回原路径。即使调用方省略哈希，保存仍会用打开或上次成功保存时的 SHA 核对磁盘文件。默认原子更新一代 `<文件>.bak`，内容为覆盖前的原始字节。可用 `--no-backup` 或 `backup:false` 跳过备份。备份失败会中止保存，随后目标写入失败时可能保留完整备份。多个 Agent 写入者通过同目录的空 `<文件>.lock` 文件协调。部分导入的工程只读。保存后当前 numeric ID 继续有效，重开时按新会话重新分配。

#### 持久对象标识

`project_snapshot` 为每个电路、组件和导线同时返回 numeric `id` 和持久 `agent_id`。例如，Pin 快照包含 `{"id":3,"agent_id":"c-0123456789abcdef0123456789abcdef","kind":"Pin"}`。移动、更新或删除组件，删除导线，更新插件实例，以及重命名、删除电路或设置主电路时，`id` 和 `agent_id` 必须且只能传一个。跨会话编辑推荐直接传入已保存的 `agent_id`。若同时传入 `circuit`，它必须与对象所属电路一致。新快照可用于查询每个持久 ID 对应的当前数字 ID。

例如，即使重开后 Pin 的数字 ID 已变化，`{"op":"update_component","agent_id":"c-0123456789abcdef0123456789abcdef","properties":{"label":"DATA"}}` 仍会更新该 Pin。每次添加对象都会在对应的 change 中返回两种 ID。导线归一化后，`wire_ids` 与 `wire_agent_ids` 按相同顺序列出存活线段，首对即为主线段的 `id` 和 `agent_id`。若同一批操作随后删除这条导线或其所属电路，添加结果会保留创建时的两种 ID，存活线段列表为空。在相同会话状态下重复预演或应用同一批操作，得到的 ID 一致。

可选 XML 属性 `agent-id` 是 logisim-rs 文档扩展。电路、组件和导线分别用 `k-`、`c-` 和 `w-` 前缀，后接 32 个小写十六进制字符。打开旧文件时在内存中分配缺失标识，下次显式保存时写入工程，复制已保存的文件会保留标识。备份仍是保存前的原始字节。导线分段时新增线段取得新标识，合并时保留一个存续标识。

无效标识或重复项会产生警告。保留首次出现的合法标识，受影响的对象获得新标识。若导入时没有内容省略，可以保存修复结果。外部工具可能忽略此属性，也可能在保存时丢弃它。Logisim-ITA 对该属性的处理尚未验证。

#### Wasm 插件实例

运行 `logisim-agent plugin catalog --wasm plugins/input/input.wasm`，或向 `plugin_catalog` 传入 `{"path":"plugins/input/input.wasm"}`，即可查看模块清单、端口、动作和属性默认值。每个工具的 `properties` 描述布尔、整数和选项控件，`property_schema` 描述补丁接受的字符串。例如队列容量 8 写作 `"8"`。已加载实例的快照返回同一份属性 schema。

下列补丁在电路 1 中放置键盘。模块路径相对于工程文件，成功后可从 `changes[0]` 取得新实例的 `id` 和 `agent_id`。

```json
{"ops":[{"op":"add_plugin","circuit":1,"position":[100,100],"tool":"keyboard","source":{"plugin":"org.logisim-rs.input","version":"1.0.0","path":"plugins/input/input.wasm"},"properties":{"capacity":"8"}}]}
```

再次放置同一依赖时，可以省略 `source`，在操作顶层指定已记录的 `plugin` 和 `version`。设置 `--root` 后，显式提交的主路径、备用路径及目录查询路径均须位于根目录内。工程原有依赖仍优先从工程目录解析相对路径，随后尝试绝对备用路径。为已记录的插件版本提供另一位置时，两处模块的目标工具定义必须一致，工程保留原来源。

将返回的 ID 传给 `update_plugin` 可单独修改插件 `properties`，也可修改文档字段 `label`、`facing`、`font` 和不透明 RGBA `color`。使用 `move_component` 移动、`remove_component` 删除。删除最后一个实例后仍保留工程依赖。模块缺失或与已保存的定义不匹配时，快照包含 `plugin_error`，仍可读取已保存的属性值、移动或删除实例。补丁失败时整批回滚。根电路实例的运行时动作由 `simulation_plugin_action` 调用。

## 已知限制

- Joystick 的标签字体不写入现有 `.circ` 格式；重开后保持固有字号 12，即使配置了其他默认标签字号。
- 深色模式下，自定义封装会增加对比度轮廓；插件内部仍由插件作者绘制，宿主只增加元件外围边界。
- Print 使用系统默认打印机。本版本未验证实体出纸及 Windows/macOS 的打印处理器；已进入系统打印队列的作业须从队列取消。
- 无界面 Agent 在 Linux x86-64 上做过快速验证；Windows/macOS 的 Agent 由同一流程构建，但尚未在对应系统上人工验证。
- 界面在 Linux X11 上做过人工验证；本版本尚未在 Windows、macOS 和原生 Wayland 上人工验证界面。

## 许可证

原创代码采用 [MIT](LICENSE)。
