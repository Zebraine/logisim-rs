# logisim-rs

Rust＋egui 数字逻辑电路工具，以 Logisim-ITA v2.16.2.2 的实际运行行为为兼容目标，
采用 Cleanroom 方式独立实现。当前支持桌面布局、画布导航及基本电路编辑。
编辑内容仅保存在当前窗口内存中；文件保存/导入与仿真尚未实现，对应入口保持禁用。

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

## 基本编辑

- 可放置 NOT、Buffer、AND、OR、NAND、NOR、XOR、XNOR、Pin、文字和 Tunnel。
- 工具栏/元件树选择工具，点击画布放置；左侧编辑标签、方向、位宽、输入数量、字体和位置。
  当前门支持 2–8 个输入（NOT/Buffer 为单输入），位宽属性为 1–32；这些属性尚不参与信号求值。
- 从端口拖出导线，或用 Wire 工具拖动/点击两点布线；Shift 切换折弯，Escape 取消。
  端点与导线相接形成分支；无端点的十字交叉不连接。选中导线后可拖动端点调整路径。
- 单击选择，Shift 多选，空白处拖动框选；拖动或方向键移动。移动元件会调整相接导线。
  选中导线时，青色显示相连网络，包括同名、同位宽的隧道；标签区分大小写。

快捷键：V 选择、W 连线、T 文字、U 隧道、R 旋转、Delete 删除；
Ctrl+A 全选，Ctrl+C/X/V 复制/剪切/粘贴，Ctrl+D 重复，Ctrl+Z 撤销，Ctrl+Shift+Z 重做。
复制粘贴当前限同一窗口；文本框获得焦点时优先处理文字编辑快捷键。

Git 只保存最小构建及根目录必要文档。设计、规格、实验和参考资料保留在本地，
不参与构建。提交范围见 [CONTRIBUTING.md](CONTRIBUTING.md)，
协作及 Cleanroom 约定见 [AGENTS.md](AGENTS.md)。

本项目原创内容采用 [MIT](LICENSE)；外部资料不属于该许可范围。
