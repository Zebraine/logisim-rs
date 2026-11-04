# logisim-rs

Rust＋egui 数字逻辑电路工具，以 Logisim-ITA v2.16.2.2 的实际运行行为为兼容目标，
采用 Cleanroom 方式独立实现。当前为 M0 工程骨架，尚无产品功能。

## 项目结构

```text
apps/logisim-desktop/    应用入口与组装（logisim-rs）
crates/logisim-core/     电路领域模型
crates/logisim-sim/      无界面仿真及元件行为
crates/logisim-file/     .circ 导入、导出与诊断
crates/logisim-ui/       egui 编辑器与画布
```

## 检查骨架

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
```

使用 Rust 2024，声明最低 Rust 1.95，工具链暂用 stable；已在 1.97.1 检查。
仅启用本地依赖，egui/eframe 0.37.0 尚未接入；运行空入口会直接退出。

Git 只保存最小构建及根目录必要文档。设计、规格、实验和参考资料保留在本地，
不参与构建。提交范围见 [CONTRIBUTING.md](CONTRIBUTING.md)，
协作及 Cleanroom 约定见 [AGENTS.md](AGENTS.md)。

本项目原创内容采用 [MIT](LICENSE)；外部资料不属于该许可范围。
