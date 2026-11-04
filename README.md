# logisim-rs

Rust＋egui 数字逻辑电路工具，以 Logisim-ITA v2.16.2.2 的实际运行行为为兼容目标，
采用 Cleanroom 方式独立实现。当前支持桌面布局、画布、电路编辑、基础组合逻辑仿真
及已实现元件的 `.circ` 读取和保存；时钟、时序及层级电路尚未实现。

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
# 启动时打开文件（也可通过 File 菜单打开）
cargo run -p logisim-rs --locked -- path/to/circuit.circ
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
- 工具栏/元件树选择工具，点击画布放置；左侧编辑标签、方向、位宽、门尺寸、输入数量、字号和位置；文字可选择对齐方式。
  当前门支持 2–8 个输入（NOT/Buffer 为单输入），信号位宽为 1–32。
- 从端口拖出导线，或用 Wire 工具拖动/点击两点布线；Shift 切换折弯，Escape 取消。
  端点与导线相接形成分支；无端点的十字交叉不连接。选中导线后可拖动端点调整路径。
- 单击选择，Shift 多选，空白处拖动框选；拖动或方向键移动。移动元件会调整相接导线。
  选中导线时，高亮相连网络，导线颜色保留信号状态。同名隧道关联、标签区分大小写；位宽冲突会报错。

快捷键：V 选择、W 连线、T 文字、U 隧道、R 旋转、Delete 删除；
Ctrl+A 全选，Ctrl+C/X/V 复制/剪切/粘贴，Ctrl+D 重复，Ctrl+Z 撤销，Ctrl+Shift+Z 重做。
复制粘贴当前限同一窗口；文本框获得焦点时优先处理文字编辑快捷键。

## 文件读取与保存

File 菜单提供 New、Open、Save 和 Save As；快捷键依次为 Ctrl+N、Ctrl+O、Ctrl+S、
Ctrl+Shift+S，Ctrl+Q 退出。文件选择框可浏览目录或输入完整路径，保存时自动补 `.circ`。
标题中的 `*` 表示未保存修改；新建、打开或退出前可保存、放弃或取消。
保存失败不会清除修改状态，检测到文件被外部修改时要求另存为。

当前互通范围：主电路中的八种门、1–32 位两态输入/输出 Pin、正交导线、文字及普通 Tunnel；
包括门尺寸、2–8 个输入、方向、标签、字号、颜色和文字对齐。导入保留实际坐标及连接端口。
多行文字数据可往返，但参考程序与本程序的排版不同；字体使用本程序的默认字体，
不承诺字体族、字形覆盖或样式一致，隧道采用固定文字色。

子电路、时序元件、三态输入、输入取反等未实现元件/电气属性会跳过并报告；
其他不支持的项目内容或外观设置也会报告。含省略内容的文件标记为 `partial`，
禁止覆盖原文件，必须另存为其他路径；只写出已支持的主电路内容。
不保存仿真输入的当前值、暂停状态、其他电路、库/工具栏自定义和未实现部分。
当前文件上限为 8 MiB、主电路最多 4096 项，不接受 DTD。

## 组合逻辑仿真

输入 Pin 默认 0；输出 Pin 只读取网络。在属性中勾选 Output pin 创建输出。
P 切换 Poke，点击单比特输入切换 0/1；多位输入在左侧用十六进制编辑后按 Enter/Apply，
也可逐位点击。导线和端口区分低、高、未知 X、错误 E、位宽不匹配；悬停查看完整二进制值。
左侧错误提示可选中相关元件和导线。

| 快捷键 | 动作 |
| --- | --- |
| P | Poke 工具 |
| 0 / 1 | 设置选中的输入值 |
| Ctrl+E | 启用 / 暂停仿真 |
| Ctrl+I | 暂停时传播一步 |
| Ctrl+R | 输入复位为零，重新传播；保留启用/暂停状态 |

暂停时修改输入会排队，单步依次推进网络和门输出。仿真操作不进入电路编辑撤销历史；
文本框获得焦点时不会触发上述全局快捷键。编辑电路后重建网络，保留仍有效的输入值。

多输入 XOR 默认按“恰好一个输入为 1”求值，XNOR 取反。
完全未接线的门输入与接了悬空导线的输入不同；全空门报错（Buffer 保持 X）。
求值有传播预算，超限暂停并提示检查反馈，可修改输入/电路或复位恢复。
当前覆盖已有组合元件，不承诺参考程序的全部传播延迟、时序或反馈行为兼容。

Git 只保存最小构建及根目录必要文档。设计、规格、实验和参考资料保留在本地，
不参与构建。提交范围见 [CONTRIBUTING.md](CONTRIBUTING.md)，
协作及 Cleanroom 约定见 [AGENTS.md](AGENTS.md)。

本项目原创内容采用 [MIT](LICENSE)；外部资料不属于该许可范围。
