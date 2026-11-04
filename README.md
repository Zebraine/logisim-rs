# logisim-rs

Rust＋egui 数字逻辑电路工具，以 Logisim-ITA v2.16.2.2 的实际运行行为为兼容目标，
采用 Cleanroom 方式独立实现。当前已实现桌面 UI 布局和画布导航；
元件编辑、文件读写与仿真尚未实现，相应入口保持禁用。

## 项目结构

```text
apps/logisim-desktop/    应用入口与组装（logisim-rs）
crates/logisim-core/     电路领域模型
crates/logisim-sim/      无界面仿真及元件行为
crates/logisim-file/     .circ 导入、导出与诊断
crates/logisim-ui/       egui 编辑器与画布
```

## 运行与检查

```sh
cargo run -p logisim-rs --locked
cargo fmt --all -- --check
cargo fetch --locked
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
```

使用 Rust 2024，声明最低 Rust 1.95，工具链暂用 stable；已在 1.97.1 检查。
界面基于 egui/eframe 0.37.0 和 wgpu，启用 Wayland/X11 后端；已在 Linux X11 环境运行验证。

布局保留顶部菜单/工具栏、左上项目树、左下属性表及缩放区、右侧画布。
拖动分隔线可调整侧栏宽度和上下比例。滚轮缩放，中键或空格＋左键拖动平移；
左下可输入缩放比例、开关网格，Home 按钮或 Ctrl+0 复位。画布右键也可复位。

Git 只保存最小构建及根目录必要文档。设计、规格、实验和参考资料保留在本地，
不参与构建。提交范围见 [CONTRIBUTING.md](CONTRIBUTING.md)，
协作及 Cleanroom 约定见 [AGENTS.md](AGENTS.md)。

本项目原创内容采用 [MIT](LICENSE)；外部资料不属于该许可范围。
