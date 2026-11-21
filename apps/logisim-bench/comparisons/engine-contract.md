# Wasm 引擎契约与启动成本

## 桌面与 Agent 的引擎契约

ABI 4 是受支持的显式插件契约。SDK 的普通出口继续生成 ABI 3，只有声明引擎集合的模块使用 ABI 4。计量单位属于所选引擎，宿主为每次请求补充 10M fuel，保留既有批次提交、暂停、Reset 和资源上限。

六个随附模块使用 ABI 4 / both，并保留插件身份、组件版本、工具定义和主模块相对路径。工程绑定的是路径、插件身份、版本和工具定义；实际加载的 Wasm 决定 ABI。包中另存六份原版 ABI 3 模块，供固定旧计量或旧宿主使用。选择 Wasmtime 时，加载旧模块的桌面会显示实际使用 wasmi 及原因。ABI 4 引擎不匹配时拒绝加载。

Linux、Windows 和 macOS 的桌面打包配方均编入双引擎，默认使用 wasmi。启用 Wasmtime 需确认并重启。普通 Cargo 构建与独立 Agent 使用 wasmi。磁盘缓存目前只信任符合权限校验的 Linux 本地目录，其他平台从源 Wasm 编译。

Stage 13 在 Linux x86-64 上做了三轮独立进程复测。Register File 空缓存与命中缓存的首开中位数分别为 453.62 ms 和 9.68 ms，课程 CPU 分别为 467.37 ms 和 23.23 ms。两条轨迹的 digest 均与 wasmi 一致，诊断数均为零。X11 实窗在重启后显示 ABI 4 使用 Wasmtime，换回同一工程路径上的原版 ABI 3 模块后显示回退 wasmi 及原因。

此次 Linux 本地压缩包为 20,070,975 字节，比 Stage 12 双引擎包增加 12,524 字节。解包验证覆盖六对新旧模块和包内 Agent。Windows 和 macOS 已更新打包配方，本轮只在 Linux 实机验收。

## ABI 4 声明与执行

SDK 的 `export_plugin!(execution = Both, …)` 生成版本化入口和 Describe 响应。也可选 `Wasmi` 或 `Wasmtime`。wire manifest 使用 `"abi":4,"execution":{"fuel":"engine-specific","engines":"both"}`，引擎集合允许 `wasmi`、`wasmtime`、`both` 三个值。预算由宿主决定。未知字段、枚举、缺少声明和静态入口冲突均拒绝。已有 ABI 1–3 的 Wasm 和 manifest 保持原样；用新版 Rust API 构造旧 manifest 时填写 `execution: None`，序列化时省略此字段。

加载器先读 Wasm 导出表。旧入口 `logisim_abi` 只走 wasmi，选择 Wasmtime 时在 stderr 报告回退。ABI 4 必须恰有一个对应入口 `logisim_abi4_wasmi`、`logisim_abi4_wasmtime` 或 `logisim_abi4_both`。宿主在编译、访问机器码缓存和执行 start 前检查声明。选中的引擎不在声明内则拒绝；加载后还须核对版本函数、manifest 和工程保存的定义。不支持 ABI 4 的旧宿主找不到旧入口。Agent 始终请求 wasmi。

每次 outputs 或 transition 请求补充该引擎的 10M fuel，缓冲区准备和分派共享预算。宿主只提交成功且通过校验的整批状态。耗尽时，该相位没有可提交结果；再次调用返回原故障，Reset 重建 VM 后重试。不同引擎的指令计量和原始错误文本可以不同。缓存只复用 Wasmtime 编译产物，每个实例的状态、预算与故障独立。

### Linux 对照结果

i5-12600KF，逻辑核 15，Rust 1.99.0 release。每个负载五轮，每轮分别启动四个独立进程，进程内预热 3 条轨迹、计量 20 条。ABI 3 使用随附的 course 模块，ABI 4 使用相同业务源码显式构建的模块（SHA-256 前缀分别为 `075dfd27` 和 `eceee0b4`）。首开统计从读取工程到首次 settle，p95 取五次首开的最近秩，等于本组最大值。热态数据取各进程统计的中位数。

| 负载 | ABI / 引擎 | 首开 p50 / p95 (ms) | 热 settle p50 / p95 (ms) | tick/s |
| --- | --- | ---: | ---: | ---: |
| Register File | 3 / wasmi | 9.41 / 9.46 | 0.114 / 0.327 | 3,048 |
| Register File | 4 / wasmi | 9.42 / 9.55 | 0.113 / 0.324 | 3,061 |
| Register File | 4 / Wasmtime，空缓存 | 458.86 / 475.29 | 0.009 / 0.027 | 35,895 |
| Register File | 4 / Wasmtime，跨进程命中 | 9.75 / 10.20 | 0.009 / 0.028 | 35,496 |
| single-cycle | 3 / wasmi | 26.03 / 26.42 | 0.149 / 0.897 | 2,275 |
| single-cycle | 4 / wasmi | 26.05 / 26.30 | 0.149 / 0.890 | 2,317 |
| single-cycle | 4 / Wasmtime，空缓存 | 481.94 / 495.93 | 0.038 / 0.261 | 8,387 |
| single-cycle | 4 / Wasmtime，跨进程命中 | 24.92 / 25.50 | 0.038 / 0.260 | 8,391 |

两条成功轨迹均为零诊断。Register File 的 digest 为 `a2689cffb70ff825`（512 tick / 1536 step）；CPU 为 `73171884c0465aad`（128 / 771）。10M fuel 下，100 万次循环在两种引擎均完成；200 万次在 wasmi 两相均完成，Wasmtime 两相均耗尽。无限循环在两种引擎均耗尽。测试核对失败批次未提交、错误可见、暂停和 Reset 后的行为。缓存损坏、不可写和两个并发进程的恢复均在 ABI 4 模块复测。

这组测量使用 ABI 4，旧插件与 Agent 仍走 wasmi。Linux 热态收益足以支持长时间运行；空缓存仍有近半秒编译成本，CPU 跨进程首开的 p95 超过 25 ms 目标。Windows/macOS、更多第三方插件及长尾首开仍待验证。

## 历史实验

下文记录阶段 2/3 在当时提交上的规则与实测数字。当前版本使用上面的 ABI 4 入口，复测命令见 CONTRIBUTING.md。

**阶段 2 决定：旧 ABI 继续使用 wasmi 1.1.0。** Wasmtime 36.0.20 可以在显式对照构建中执行同一插件。对旧插件开放运行时切换会改变部分有限计算的完成、暂停和状态提交结果。Stage 3 保留生产引擎。若产品选择为长时间仿真引入另一种计量契约，再考虑新 ABI。

阶段 3 在显式构建的桌面应用中增加了需确认、重启后生效的选项。旧插件使用 Wasmtime 时接受下表的计量差异。阶段 3 的磁盘缓存仅用于基准实验。当前 Linux 桌面可用 `wasmtime-cache` 启用私有缓存，复测见 [CONTRIBUTING.md](../../../CONTRIBUTING.md)。默认桌面与 Agent 继续使用 wasmi。

## 可观察的资源契约

fuel 是每次请求的执行预算。宿主在请求开始时补充预算，guest 缓冲区分配和此次分派共享预算。每条指令如何记账由引擎决定。四态输出相同只能验证成功轨迹，不能证明资源边界相同。

| 同一模块、同一相位 | wasmi 10M | Wasmtime 10M | Wasmtime 20M |
| --- | --- | --- | --- |
| 1k、10k、100k、1M 次递减 | 完成 | 完成 | 完成 |
| 2M 次递减，outputs | 完成，消耗 6,000,479 | 耗尽，余 0 | 完成，消耗 10,000,018 |
| 2M 次递减，transition | 返回下一状态，消耗 6,000,017 | 耗尽，余 0 | 返回相同状态，消耗 10,000,018 |
| 3.6M 次递减 | 耗尽 | 耗尽 | 完成 |
| 无限循环 | 耗尽 | 耗尽 | 耗尽 |

10M 无限循环在 wasmi 中还剩 1 fuel，已不足以执行下一计量块；Wasmtime 报余 0。20M 会放过旧引擎在 10M 下暂停的作业。在真实寄存器文件中，outputs 平均消耗分别为 100,922 与 113,421 fuel，transition 为 44,199 与 88,746。两相倍率约 1.12 和 2.01，递减循环约 1.67。固定模块和固定输入的实测可重现；按模块指定一个倍数仍无法保留所有请求的边界。矩阵逐例保存模块 SHA-256、相位、预算、实际余量、返回值或错误、重试分派次数和复位重放结果。

| 边界 | 两个实际后端的验证与用户影响 |
| --- | --- |
| 模块与实例化 | 只接收至多 4 MiB 的 Wasm。原型先用 wasmi 校验原始字节，拒绝 host/WASI 导入；缺失导出、签名错误、start trap 都中止装载。无运行状态提交。 |
| 内存、表 | 同一 Store 限制单 memory、table、instance，16 MiB 线性内存、10,000 表项。检查初始上限、超限实例化与运行时增长失败。 |
| 消息与状态 | JSON 和协商后的线性帧共用 256 KiB 消息及 64 KiB 显式状态上限；保留范围、形状和有效值检查。识别到的坏帧报错。课程轨迹两相走线性帧；JSON 正常、错误和 commit 由独立 Wasm 用例覆盖。 |
| trap 与错误 | 宿主产生的身份、导入及消息限制错误相同。引擎的导出、内存、unreachable 和 fuel 文本可见差异。原型保留 Wasmtime 回溯和具体原因。应用不得按回溯文字相等判断契约等价。 |
| outputs / transition | 正常 outputs 返回驱动与图形，transition 返回待提交状态。两相任一耗尽均返回插件故障，故障元件显示 E。transition 失败丢弃整批暂存值，包括其他实例的采样；已完成的前一批状态保留。 |
| 暂停、重试、reset | headless settle 直接返回故障；桌面 worker 与 Agent 负责暂停。VM 故障后的裸重试返回已保存错误，不再进入 guest。reset 重建实例，固定轨迹重新执行，仍超过预算时再次失败。没有自动加额或改走事件模式。 |

## 冷启动与缓存

Linux x86-64，i5-12600KF 的逻辑核 15，Rust 1.99.0 release；寄存器文件和课程 CPU 均使用 SHA-256 为 `075dfd270b425125dc60e9e63eab5aad3d724cdeb2622b39d9f43c9f446fc323` 的 course.wasm。两引擎的 digest 分别为 `a2689cffb70ff825`、`73171884c0465aad`，每轮分别执行 512、128 tick，诊断均为零。10M 至 80M 的 Wasmtime 轨迹均与 10M wasmi 一致。

| 工程 / 引擎（10M） | 首开 p50 / p95，ms | 热 settle p50 / p95，ms | 热 tick/s | 三进程吞吐极差 / 中位数 |
| --- | ---: | ---: | ---: | ---: |
| 寄存器 / wasmi | 9.73 / 10.86 | 0.1143 / 0.3224 | 3,034 | 0.6% |
| 寄存器 / Wasmtime | 468.43 / 473.90 | 0.0096 / 0.0277 | 35,053 | 0.2% |
| 课程 CPU / wasmi | 27.73 / 27.79 | 0.1756 / 0.9193 | 2,215 | 1.2% |
| 课程 CPU / Wasmtime | 492.77 / 509.76 | 0.0426 / 0.2790 | 7,843 | 3.9% |

| 冷路径分项，中位 ms | 读 Wasm | 额外 wasmi 准入校验 | 引擎验证 / 编译 | VM 创建累计 | contract 查询累计 | 首次 settle |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 寄存器 / wasmi | 0.170 | — | 2.893 | 1.303 | 0.320 | 0.704 |
| 寄存器 / Wasmtime | 0.174 | 2.337 | 464.722 | 0.170 | 0.045 | 0.071 |
| CPU / wasmi | 0.165 | — | 2.880 | 2.233 | 0.535 | 2.082 |
| CPU / Wasmtime | 0.165 | 2.404 | 473.920 | 0.309 | 0.107 | 0.756 |

| 重开策略 | 寄存器 p50 / p95，ms | CPU p50 / p95，ms | 每个工程的命中 |
| --- | ---: | ---: | --- |
| wasmi 复用 Module | 3.19 / 3.22 | 18.18 / 18.59 | 每进程首开各 miss，之后 60/60 hit |
| Wasmtime 新 Engine | 466.33 / 475.27 | 507.31 / 516.17 | 0/60 |
| Wasmtime 复用 Engine | 465.80 / 469.80 | 506.14 / 536.93 | 0/60 |
| Wasmtime 复用 Module | 1.02 / 1.08 | 16.24 / 16.62 | 每进程首开各 miss，之后 60/60 hit |
| Wasmtime 空磁盘缓存，首次创建 | 485.04 / 494.36 | 522.28 / 523.40 | 0/3 |
| Wasmtime 已填充磁盘缓存，新进程首开 | 8.60 / 8.65 | 25.61 / 25.64 | 3/3 |
| 上一行进程内继续打开，每次新建 Engine | 8.00 / 8.51 | 23.12 / 23.45 | 60/60 |

本轮采用的交互评估门槛为首次打开 p95 ≤100 ms、已编译模块重开 p95 ≤25 ms。两个工程在首次编译时均未达标。内存命中均达标；跨进程磁盘命中时课程 CPU 首开为 25.64 ms。这里的命中率来自重复打开相同字节，不能用作用户工程的预期命中率。

冷启动差额除以两个引擎的每 tick 耗时差，得到无缓存打开后的摊平点约为寄存器 **1,524 tick**、CPU **1,436 tick**。比这更短的冷作业总耗时以 wasmi 为优；长作业可摊平编译成本，打开时的等待仍然存在。

计时从读工程至首次 stable settle，文件系统页缓存已热。首次打开每种组合有三个独立进程，p95 为三个样本的最大值。每个进程之后的 20 次重开合并计算最近秩，共 60 次；每次均重新读取字节、检查保存的身份和元件定义、建 Store/VM，并在计时外复核完整四态轨迹。热调用按每进程 3 轮预热、20 轮测量取样，吞吐取三个进程的中位数。各层计时有嵌套，不能相加。

- **进程内。** 单条有界缓存键为精确源字节，编译版本、目标和配置固定在同一 Engine 生命周期内。换模块会替换旧条目；相同路径替换文件内容不会误命中。Module 可以复用，实例、状态、contract 查询和项目定义验证仍然独立。
- **跨进程。** 实验使用 Wasmtime 自带编译缓存的安全 Rust 入口。固定版本按原始 Wasm、编译器标志、目标 triple、ISA 特性、tunables、Wasm features 及 module version 区分条目，并以编译器构建身份划分目录。宿主仍校验原始 Wasm。缓存存储的是原生可执行代码；哈希键和格式检查不能认证供应者。
- **权限、损坏、并发。** Runner 在系统临时目录创建全新的私有目录，每轮使用独立空间。Linux 入口核对私有目录、属主、祖先和符号链接。使用原子替换。目录不可用时从源编译；不完整的压缩记录会触发重编译；变更编译选项引起 miss。测试包括并发写同一键及恢复后的命中。实验信任运行用户及其父目录；同账号恶意篡改不属于该信任边界。不要从工程、下载项或共享可写目录接收 native cache。直接调用 `Module::deserialize{,_file}` 需要 `unsafe` 和受信的机器码，自有代码没有采用。
- **分发。** 自带插件若采用预编译，需要按 Wasmtime 构建、目标、CPU 基线及选项发布已认证产物，验证后才可装载。外部插件始终可从原始 Wasm 编译。签名、三平台分发和更新策略未验收，维持 Wasm 分发。原型仅在 Linux 核实私有磁盘缓存；Windows 的 ACL/替换与 macOS Apple Silicon 的 ISA、JIT 权限及签名需分别验证，跨平台不共享 native 条目。

## ABI 与入口选择

| 方案 | 入口与后果 | 决策 |
| --- | --- | --- |
| A 保留 wasmi | ABI 1–3 维持现行计量。研究用二进制显式启用 `compare-wasmtime`；两个引擎需要分别跑契约矩阵。 | 采用。生产构建沿用原入口。 |
| B 新 ABI 和显式能力 | 新 ABI 的 manifest 声明可接受的计量模型及版本，loader 在执行用户行为前检查能力。旧 ABI 强制 wasmi。规范需定义预算重置范围、超限分类、阶段提交及 reset；不支持声明时报告原因。 | 若要提供面向用户的加速，先批准此契约，再实现及测试 loader/SDK。 |
| C 全量切换 | 需产品明确接受旧作业完成与暂停边界变化，定义错误兼容级别，满足首次打开预算，在三平台完成构建、实测与回退演练。回退只能发生在实例创建前。 | 暂不采用。热态收益无法解决语义差异。 |

## 阶段 3 双引擎复核

Linux x86-64，固定逻辑核 15。对同一 release 基准二进制分别传入 `--wasm-engine wasmi` 和 `--wasm-engine wasmtime`。每种组合启动三个进程，每个进程预热 3 轮、测量 20 轮。下表取三个进程的中位数。首开从读取工程到首次收敛，系统文件页可以已在内存中；热 settle 列取各进程的相应分位数。桌面不启用原生磁盘缓存。

| 负载 | 引擎 | 首开 ms | 热 settle p50 / p95，ms | tick/s |
| --- | --- | ---: | ---: | ---: |
| 寄存器文件 | wasmi | 9.65 | 0.1166 / 0.3240 | 2,993 |
| 寄存器文件 | Wasmtime | 462.26 | 0.0095 / 0.0276 | 35,306 |
| 课程 CPU | wasmi | 26.43 | 0.1496 / 0.9080 | 2,239 |
| 课程 CPU | Wasmtime | 474.12 | 0.0415 / 0.2689 | 8,081 |

寄存器文件的 512 tick 得到 `a2689cffb70ff825`，课程 CPU 的 128 tick 得到 `73171884c0465aad`。两个引擎的 steps、插件工作量和四态摘要逐项一致，诊断为零。研究用私有磁盘缓存单独复核：每个工程用独立空目录打开四次，各一次 miss、三次 hit。随后新进程四次全部命中，首次打开寄存器文件 8.68 ms、CPU 26.95 ms。这些命中耗时仅适用于实验缓存。

## 复测

从仓库根目录在 Linux release 下运行。首次准备可选依赖后可离线重放。选择可用的独占逻辑核，将下面的 15 替换成该核编号。

```sh
out=$(mktemp -d)
cargo build --release --locked --offline -p logisim-bench --features engine-study
cp target/release/logisim-bench "$out/wasmi"
cargo build --release --locked --offline -p logisim-bench --features compare-wasmtime
cp target/release/logisim-bench "$out/wasmtime"
python3 apps/logisim-bench/comparisons/engine-study.py --cpu 15 \
  --wasmi "$out/wasmi" --wasmtime "$out/wasmtime" --out "$out/results"

cargo test --release --locked --offline -p logisim-plugin --features engine-study -- --nocapture
cargo test --release --locked --offline -p logisim-plugin --features engine-study,compare-wasmtime -- --nocapture
```

`FUEL` 记录包括 10M、20M、40M、80M 的逐次读数；`LIMIT` 记录保留原错误。基准原始 JSON 内 `opens[].spans`、`first_settle_ms` 及 `plugin_phases.hot` 分别给出装载/实例化/契约查询、首次收敛及两个热相位。Runner 核验默认依赖树、二进制引擎、轨迹和工作量，并记录环境和输入 SHA-256。工作区的 all-features 矩阵在同一进程内测试两种后端；普通插件加载入口使用 wasmi。原生缓存的上游说明见 [Wasmtime cache](https://docs.wasmtime.dev/cli-cache.html)。

## 实验 A · 应用私有模块缓存

Linux x86-64，i5-12600KF，Rust 1.99.0，Wasmtime 36.0.20。用同一 release 基准和 course 模块，主进程固定 CPU 15。每个工程使用 5 个独立空目录，每次首次打开后在同进程重开 3 次。再用第二、第三个新进程打开，合并得到各 10 个跨进程命中样本。下表从读工程计到首次 settle，单位 ms，列内为中位数 / 最近秩 p95。

| 工程 | 关闭缓存 | 空缓存 | 同进程重开 | 跨进程命中 |
| --- | ---: | ---: | ---: | ---: |
| register-file | 447.17 / 459.25 | 455.28 / 468.05 | 7.41 / 8.28 | 9.51 / 9.78 |
| single-cycle | 462.57 / 473.11 | 463.27 / 473.36 | 20.77 / 22.09 | 24.35 / 24.84 |

两个工程共用 course.wasm 的 687,281 字节缓存记录。截断、只读及并发首次发布均通过完整四态轨迹检查。跨进程命中 p95 在该组样本中达到 25 ms 目标；并发写入后的独立 CPU 重开 p95 为 25.34 ms。寄存器空缓存首开中位数比关闭缓存增加 8.11 ms。以上数据记录了 ABI 4 定案前的缓存成本。两种引擎的预算计量仍有差异，默认引擎保持 wasmi。Linux 私有目录、手动清理及复测命令见 [CONTRIBUTING.md](../../../CONTRIBUTING.md)。
