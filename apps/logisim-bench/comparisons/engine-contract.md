# Stage 2 · Wasm 引擎契约与启动成本

**决定：旧 ABI 继续使用 wasmi 1.1.0。** Wasmtime 36.0.20 可以在显式对照构建中执行同一插件。对旧插件开放运行时切换会改变部分有限计算的完成、暂停和状态提交结果。Stage 3 保留生产引擎。若产品选择为长时间仿真引入另一种计量契约，再考虑新 ABI。

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

`FUEL` 记录包括 10M、20M、40M、80M 的逐次读数；`LIMIT` 记录保留原错误。基准原始 JSON 内 `opens[].spans`、`first_settle_ms` 及 `plugin_phases.hot` 分别给出装载/实例化/契约查询、首次收敛及两个热相位。Runner 核验默认依赖树、二进制引擎、轨迹和工作量，并记录环境和输入 SHA-256。工作区的 all-features 矩阵会显式选择实验后端；默认桌面与 Agent release 构建不启用该特性。原生缓存的上游说明见 [Wasmtime cache](https://docs.wasmtime.dev/cli-cache.html)。
