//! Presentation-only translations. Circuit data and file-format identifiers never pass through here.
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    English,
    SimplifiedChinese,
}
impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::SimplifiedChinese];
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::SimplifiedChinese => "简体中文",
        }
    }
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.code() == code)
    }
}
thread_local! { static LANGUAGE: Cell<Language> = const { Cell::new(Language::English) }; }
/// A rendering scope, restored even on unwind. Separate windows/threads cannot leak languages.
/// Hosts should also scope their own file dialogs and simulation presentation.
pub fn with_language<R>(language: Language, render: impl FnOnce() -> R) -> R {
    struct Restore(Language);
    impl Drop for Restore {
        fn drop(&mut self) {
            LANGUAGE.set(self.0);
        }
    }
    let _restore = Restore(LANGUAGE.replace(language));
    render()
}
/// Translate an application-owned label. Never use this on user-supplied names or text.
pub fn tr(english: &str) -> &str {
    if LANGUAGE.get() == Language::English {
        return english;
    }
    MESSAGES
        .binary_search_by_key(&english, |(key, _)| *key)
        .map_or(english, |i| MESSAGES[i].1)
}
/// Positional placeholders keep inserted user data opaque, including braces and translation keys.
pub fn format(english: &str, arguments: &[&dyn std::fmt::Display]) -> String {
    let mut parts = tr(english).split("{}");
    let mut result = parts.next().unwrap_or_default().to_owned();
    for (i, part) in parts.enumerate() {
        if let Some(argument) = arguments.get(i) {
            use std::fmt::Write;
            let _ = write!(result, "{argument}");
        } else {
            result.push_str("{}");
        }
        result.push_str(part);
    }
    result
}
#[macro_export]
macro_rules! tf {
    ($key:literal, $($value:expr),* $(,)?) => {{
        const _: () = assert!($crate::i18n::placeholder_count($key) == [$(stringify!($value)),*].len());
        $crate::i18n::format($key, &[$(&$value),*])
    }};
}

#[doc(hidden)]
pub const fn placeholder_count(template: &str) -> usize {
    let bytes = template.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'}' {
            count += 1;
        }
        i += 1;
    }
    count
}

const MESSAGES: &[(&str, &str)] = &[
    ("  (main)", "  （主电路）"),
    (" (partial)", " （部分导入）"),
    ("(unnamed)", "（未命名）"),
    (
        "0 / 1: set selected input. Multi-bit values use hexadecimal.",
        "0 / 1：设置选中输入。多位输入使用十六进制。",
    ),
    (
        "A circuit cannot contain itself, directly or indirectly.",
        "电路不能直接或间接包含自身。",
    ),
    (
        "A cleanroom digital circuit workspace.",
        "采用 Cleanroom 方式独立实现的数字电路工作台。",
    ),
    ("AND Gate", "与门"),
    ("About logisim-rs", "关于 logisim-rs"),
    ("Actual size · 100%", "实际大小 · 100%"),
    ("Add", "添加"),
    ("Add Circuit… · Ctrl+Shift+N", "添加电路… · Ctrl+Shift+N"),
    ("Add Vertex", "添加顶点"),
    ("Add circuit", "添加电路"),
    ("Add vertex", "添加顶点"),
    ("Anchor", "锚点"),
    ("Appearance", "外观"),
    ("Appearance: {}", "外观：{}"),
    ("Apply", "应用"),
    (
        "Apply the hexadecimal value · Enter",
        "应用十六进制数值 · Enter",
    ),
    ("Arithmetic", "算术"),
    ("Ascending", "升序"),
    (
        "At most 128 circuits are supported.",
        "最多支持 128 个电路。",
    ),
    ("Back", "返回"),
    ("Base", "基础"),
    ("Baseline", "基线"),
    ("Binary", "二进制"),
    ("Bit Extender", "位扩展器"),
    ("Bit {}", "第 {} 位"),
    ("Body width ", "主体宽度 "),
    ("Bold", "粗体"),
    ("Bottom", "底部"),
    ("Branch {}", "分支 {}"),
    ("Bring to Front", "置于顶层"),
    ("Buffer", "缓冲器"),
    ("Cancel", "取消"),
    (
        "Cannot remove the last circuit or a circuit that is still used by an instance.",
        "无法删除最后一个电路或仍被实例引用的电路。",
    ),
    (
        "Cannot save through a dangling symlink.",
        "无法通过失效的符号链接保存。",
    ),
    ("Center", "居中"),
    ("Centered", "居中"),
    (
        "Changes update every instance. Wires keep their positions; reconnect moved ports in the circuit layout.",
        "修改会更新所有实例。导线保留原位置；端口移动后请在电路布局中重新接线。",
    ),
    (
        "Chinese characters require a system CJK font (for example Noto Sans CJK SC).",
        "中文显示需要系统中的中文字体（例如 Noto Sans CJK SC）。",
    ),
    (
        "Choose a different file: this source contains unsupported content.",
        "请选择其他文件：此源文件含有未支持的内容。",
    ),
    (
        "Choose a file, not a directory.",
        "请选择文件，而不是文件夹。",
    ),
    (
        "Choose a nonempty, unique circuit name (at most 1024 bytes).",
        "请输入非空且不重复的电路名称（最多 1024 字节）。",
    ),
    (
        "Choose a nonempty, unique circuit name.",
        "请输入非空且不重复的电路名称。",
    ),
    ("Choose a regular .circ file.", "请选择普通 .circ 文件。"),
    (
        "Choose a tool to place a component. Drag a terminal to connect.",
        "选择工具以放置元件，拖动端口以连线。",
    ),
    ("Circuit layout", "电路布局"),
    ("Circuit name", "电路名称"),
    ("Circuit operation unavailable", "无法执行电路操作"),
    ("Circuit: {}", "电路：{}"),
    (
        "Click to add text, then edit its content here.",
        "点击添加文字，然后在此编辑内容。",
    ),
    (
        "Click two points or drag to connect. Shift changes the bend. Esc cancels.",
        "点击两个端点或拖动连线。Shift 切换拐角，Esc 取消。",
    ),
    (
        "Click vertices. Double-click or Enter to finish. Escape cancels.",
        "点击添加顶点，双击或 Enter 完成，Esc 取消。",
    ),
    ("Clock", "时钟"),
    ("Close", "关闭"),
    ("Closed polygon", "闭合多边形"),
    ("Color", "颜色"),
    (
        "Combinational simulation is available. Supported .circ files can be opened and saved. Clocks are not yet available.",
        "支持组合逻辑仿真与已实现电路的 .circ 文件读写。尚不支持时钟。",
    ),
    (
        "Component editing, wiring and canvas navigation are available.",
        "支持元件编辑、连线与画布导航。",
    ),
    ("Connected wires: {}", "相连导线：{}"),
    ("Constant", "常量"),
    ("Control", "控制点"),
    ("Copy · Ctrl+C", "复制 · Ctrl+C"),
    ("Corner radius ", "圆角半径 "),
    (
        "Could not create a temporary save file",
        "无法创建临时保存文件",
    ),
    ("Curve", "曲线"),
    ("Custom appearance", "自定义外观"),
    ("Cut · Ctrl+X", "剪切 · Ctrl+X"),
    ("Data bits", "数据位宽"),
    ("Default appearance", "默认外观"),
    ("Delete", "删除"),
    ("Descending", "降序"),
    ("Discard changes", "放弃修改"),
    ("Distribute", "分配"),
    (
        "Double-click to edit; single-click another circuit to place an instance.",
        "双击编辑；单击其他电路以放置实例。",
    ),
    (
        "Drag empty space to select. Shift adds to the selection.",
        "在空白处拖动框选，Shift 追加选择。",
    ),
    ("Drag or type a zoom percentage", "拖动或输入缩放百分比"),
    (
        "Drag the blue endpoint handles to reshape this wire.",
        "拖动蓝色端点以调整导线。",
    ),
    (
        "Drag the two endpoints, then click the control point.",
        "拖出两个端点，再点击控制点。",
    ),
    (
        "Drag to draw. Shift-click adds to a selection.",
        "拖动绘图，Shift 点击追加选择。",
    ),
    ("Drawing style", "绘图样式"),
    ("Duplicate", "重复"),
    ("Duplicate · Ctrl+D", "重复 · Ctrl+D"),
    ("East", "东"),
    ("Edit", "编辑"),
    (
        "Edit Circuit Appearance · Ctrl+Shift+E",
        "编辑电路外观 · Ctrl+Shift+E",
    ),
    ("Edit Circuit Layout", "编辑电路布局"),
    ("Edit appearance", "编辑外观"),
    ("Ellipse", "椭圆"),
    (
        "Enable / pause simulation · Ctrl+E",
        "启用 / 暂停仿真 · Ctrl+E",
    ),
    ("Enable Simulation · Ctrl+E", "启用仿真 · Ctrl+E"),
    ("End", "终点"),
    ("End: {}, {}", "终点：{}, {}"),
    ("Enter a file name.", "请输入文件名。"),
    (
        "Enter a hexadecimal value within {} bits.",
        "请输入 {} 位范围内的十六进制数值。",
    ),
    (
        "Error signal: conflicting drivers / undefined input",
        "错误信号：驱动冲突 / 未定义输入",
    ),
    ("Exit", "退出"),
    (
        "Expanded hierarchy exceeds 65536 items.",
        "展开后的层级超过 65536 项。",
    ),
    ("Export Image…", "导出图片…"),
    ("Extension", "扩展方式"),
    ("Facing", "朝向"),
    ("Fan out", "分支数量"),
    ("File", "文件"),
    ("File exceeds the 8 MiB limit.", "文件超过 8 MiB 限制。"),
    ("File is not UTF-8: {}", "文件不是 UTF-8 编码：{}"),
    ("File name or full path", "文件名或完整路径"),
    ("File operation failed", "文件操作失败"),
    ("Fill", "填充"),
    ("First corner", "第一个角"),
    ("Font family", "字体"),
    ("Font size", "字号"),
    ("Front", "置于顶层"),
    ("Gate size", "门尺寸"),
    ("Gates", "逻辑门"),
    ("Generate package", "生成封装"),
    ("Grid", "网格"),
    ("Ground", "接地"),
    ("Help", "帮助"),
    ("Hexadecimal", "十六进制"),
    (
        "Hierarchy exceeds 32 levels or 1024 instances.",
        "层级超过 32 层或 1024 个实例。",
    ),
    ("Horizontal", "水平对齐"),
    ("Imported supported content only", "仅导入已支持的内容"),
    ("Include pin names", "包含引脚名称"),
    ("Incompatible bit widths", "位宽不兼容"),
    ("Input", "输入"),
    ("Input (hex)", "输入（十六进制）"),
    ("Input bits", "输入位宽"),
    ("Input/Output", "输入 / 输出"),
    ("Inputs", "输入数量"),
    (
        "Inputs have pending changes. Step or Run to propagate.",
        "输入有待传播的变化，请单步或运行。",
    ),
    (
        "Instance inputs are driven by the parent circuit. Edits apply to all instances.",
        "实例输入由父电路驱动。编辑会应用到所有实例。",
    ),
    ("Internal Pin: ({}, {})", "内部引脚：（{}, {}）"),
    ("Italic", "斜体"),
    ("Label", "标签"),
    ("Label location", "标签位置"),
    ("Language", "语言"),
    (
        "Language changes apply immediately and are saved for the next launch.",
        "语言切换立即生效，并在下次启动时保留。",
    ),
    ("Left", "左"),
    ("Left-handed", "左侧"),
    ("Legacy", "传统"),
    ("Line", "线段"),
    ("Load Library…", "加载元件库…"),
    (
        "Logisim-ITA compatible .circ · supported components only",
        "兼容 Logisim-ITA 的 .circ · 仅限已支持元件",
    ),
    ("Lower", "下移一层"),
    ("Lower · Ctrl+Down", "下移一层 · Ctrl+Down"),
    ("Maximize / Restore", "最大化 / 还原"),
    ("Medium", "中"),
    ("Memory", "存储"),
    ("Minimize", "最小化"),
    ("Missing circuit", "缺少电路"),
    ("Missing file name", "缺少文件名"),
    ("Missing parent directory", "缺少父文件夹"),
    ("Missing simulation root.", "缺少仿真根电路。"),
    ("Move Circuit Down", "下移电路"),
    ("Move Circuit Up", "上移电路"),
    ("Move down", "下移"),
    (
        "Move this port to arrange the package. Edit or remove its Pin in the circuit layout.",
        "移动此端口以调整封装。在电路布局中编辑或删除对应引脚。",
    ),
    ("Move up", "上移"),
    ("NAND Gate", "与非门"),
    ("NOR Gate", "或非门"),
    ("NOT Gate", "非门"),
    ("Narrow", "窄"),
    ("New · Ctrl+N", "新建 · Ctrl+N"),
    ("No tools available yet", "暂无可用元件"),
    ("None", "无"),
    ("North", "北"),
    ("OK", "确定"),
    ("OR Gate", "或门"),
    ("Octal", "八进制"),
    ("One", "补一"),
    ("Open", "打开"),
    ("Open circuit", "打开电路"),
    ("Open… · Ctrl+O", "打开… · Ctrl+O"),
    ("Opposite corner", "对角"),
    ("Outline", "描边"),
    ("Output", "输出"),
    ("Output bits", "输出位宽"),
    ("Output pin", "输出引脚"),
    (
        "P: Poke · click a 1-bit input to toggle.",
        "P：交互 · 点击单比特输入切换电平。",
    ),
    ("Package design", "封装设计"),
    ("Paste · Ctrl+V", "粘贴 · Ctrl+V"),
    (
        "Pasting would create a missing or recursive subcircuit.",
        "粘贴会产生缺失或递归引用的子电路。",
    ),
    ("Pause", "暂停"),
    ("Pause · Ctrl+E", "暂停 · Ctrl+E"),
    ("Paused", "已暂停"),
    ("Pin", "引脚"),
    ("Pin #{}", "引脚 #{}"),
    ("Pin ({}, {})", "引脚（{}, {}）"),
    ("Place instance", "放置实例"),
    ("Plexers", "复用器"),
    ("Poke", "交互"),
    (
        "Poke · P · toggle inputs / inspect signals",
        "交互 · P · 切换输入 / 查看信号",
    ),
    ("Polygon", "多边形"),
    ("Polyline", "折线"),
    ("Port spacing ", "端口间距 "),
    ("Ports", "端口"),
    ("Power", "电源"),
    ("Print…", "打印…"),
    ("Probe", "探针"),
    ("Project", "项目"),
    ("Project Options…", "项目选项…"),
    ("Propagate one batch · Ctrl+I", "传播一个批次 · Ctrl+I"),
    ("Propagation limit — paused", "达到传播上限 — 已暂停"),
    (
        "Propagation limit: check feedback, then Reset or Run",
        "达到传播上限：请检查反馈，再重置或运行",
    ),
    ("Pull Resistor", "上拉 / 下拉电阻"),
    ("Radix", "进制"),
    ("Raise", "上移一层"),
    ("Raise · Ctrl+Up", "上移一层 · Ctrl+Up"),
    ("Rect", "矩形"),
    ("Rectangle", "矩形"),
    ("Redo · Ctrl+Shift+Z", "重做 · Ctrl+Shift+Z"),
    ("Refresh", "刷新"),
    ("Remove", "删除"),
    ("Remove '{}' ?", "删除“{}”？"),
    ("Remove Circuit…", "删除电路…"),
    ("Remove Vertex", "删除顶点"),
    ("Remove circuit", "删除电路"),
    ("Remove vertex", "删除顶点"),
    ("Rename", "重命名"),
    ("Rename Circuit…", "重命名电路…"),
    ("Rename circuit", "重命名电路"),
    ("Rename…", "重命名…"),
    ("Replace", "替换"),
    ("Replace existing file?", "替换已有文件？"),
    (
        "Replace the artwork with a labeled package. You can undo this change.",
        "用带标签的封装替换外观，可以撤销。",
    ),
    ("Reset", "重置"),
    ("Reset Simulation · Ctrl+R", "重置仿真 · Ctrl+R"),
    (
        "Reset all inputs to zero · Ctrl+R",
        "将所有输入重置为零 · Ctrl+R",
    ),
    ("Reset canvas", "重置画布"),
    ("Reset canvas · Ctrl+0", "重置画布 · Ctrl+0"),
    ("Retry", "重试"),
    ("Return to Parent · Alt+Left", "返回父电路 · Alt+Left"),
    (
        "Return to parent instance · Alt+Left",
        "返回父实例 · Alt+Left",
    ),
    ("Revert to Default Appearance", "恢复默认外观"),
    ("Revert to default appearance", "恢复默认外观"),
    ("Right", "右"),
    ("Right-handed", "右侧"),
    ("Rotate", "旋转"),
    ("Rotate clockwise · R", "顺时针旋转 · R"),
    ("Rotate · R", "旋转 · R"),
    ("Round", "圆角"),
    ("Rounded rectangle", "圆角矩形"),
    ("Run", "运行"),
    ("Running", "运行中"),
    ("Save", "保存"),
    ("Save As… · Ctrl+Shift+S", "另存为… · Ctrl+Shift+S"),
    ("Save changes?", "保存修改？"),
    ("Save circuit as", "电路另存为"),
    ("Save · Ctrl+S", "保存 · Ctrl+S"),
    ("Select", "选择"),
    ("Select All · Ctrl+A", "全选 · Ctrl+A"),
    (
        "Select affected components and wires",
        "选择受影响的元件和导线",
    ),
    ("Select · V", "选择 · V"),
    (
        "Select, drag or use arrow keys to move. Drag shape handles to reshape. Ports and anchor stay on the circuit grid.",
        "选择后拖动或使用方向键移动，拖动控制点调整图形。端口与锚点始终吸附到电路网格。",
    ),
    ("Selection: {}", "选择：{}"),
    ("Selection: {} item(s)", "已选择 {} 项"),
    ("Send to Back", "置于底层"),
    ("Set as Main Circuit", "设为主电路"),
    ("Settings", "设置"),
    ("Settings need attention.", "设置需要处理。"),
    ("Show grid", "显示网格"),
    ("Showing the first 32 issues.", "仅显示前 32 个问题。"),
    ("Sign", "符号位"),
    ("Signal: {}", "信号：{}"),
    ("Signed decimal", "有符号十进制"),
    ("Simulate", "仿真"),
    ("Simulation issues ({})", "仿真问题（{}）"),
    ("Size ", "字号 "),
    ("Snap", "网格吸附"),
    ("South", "南"),
    ("Splitter", "分线器"),
    ("Start", "起点"),
    ("Start: {}, {}", "起点：{}, {}"),
    ("Step", "单步"),
    ("Step Simulation · Ctrl+I", "单步仿真 · Ctrl+I"),
    ("Subcircuit", "子电路"),
    ("TTL", "TTL"),
    ("Text", "文字"),
    ("Text color", "文字颜色"),
    ("Text · T", "文字 · T"),
    (
        "The anchor sets the placement origin and the artwork's original facing.",
        "锚点决定放置原点与外观的原始朝向。",
    ),
    (
        "The destination changed, cannot be read, or needs overwrite confirmation. Use Save As and choose the file again.",
        "目标文件已更改、无法读取或需要确认覆盖。请使用“另存为”重新选择。",
    ),
    (
        "The highlight follows the connected net, including matching tunnel labels.",
        "高亮显示整个相连网络，包括同名隧道。",
    ),
    (
        "The original file is protected. Use Save As to write the supported project subset to a different file.\n\n{}",
        "原文件已受保护。请使用“另存为”将已支持的项目内容保存到其他文件。\n\n{}",
    ),
    (
        "The shape is empty, invalid, or exceeds the appearance limit.",
        "图形为空、无效或超过外观数量限制。",
    ),
    (
        "This import omitted unsupported content. Save to a different file to protect the original.",
        "导入时省略了未支持的内容，请另存到其他文件以保护原文件。",
    ),
    (
        "This project has unsaved changes.",
        "此项目有未保存的修改。",
    ),
    (
        "This removes the circuit and its contents. You can undo this change.",
        "这会删除电路及其内容，可以撤销。",
    ),
    ("Tick Once — no clock components", "单次时钟 — 尚无时钟元件"),
    ("Toggle bit {}", "切换第 {} 位"),
    ("Tool: {}", "工具：{}"),
    ("Top", "顶部"),
    ("Transistor", "晶体管"),
    ("Transmission Gate", "传输门"),
    ("Tunnel", "隧道"),
    ("Tunnel · U", "隧道 · U"),
    ("Undo · Ctrl+Z", "撤销 · Ctrl+Z"),
    (
        "Unknown or invalid language preference.",
        "语言配置未知或无效。",
    ),
    ("Unsigned decimal", "无符号十进制"),
    ("Untitled", "未命名"),
    ("Up", "上级目录"),
    (
        "Use File to open or save the supported .circ subset. Clocks are not available yet.",
        "通过“文件”打开或保存已支持的 .circ 内容。尚不支持时钟。",
    ),
    (
        "User configuration directory is unavailable.",
        "无法确定用户配置目录。",
    ),
    ("Value (hex)", "数值（十六进制）"),
    ("Vertex {}", "顶点 {}"),
    ("Vertical", "垂直对齐"),
    ("View instance", "查看实例"),
    ("View {}", "查看 {}"),
    (
        "Viewing this instance. Edits change the shared circuit definition.",
        "正在查看此实例。编辑会修改共享的电路定义。",
    ),
    ("West", "西"),
    ("Wide", "宽"),
    ("Width ", "宽度 "),
    ("Window", "窗口"),
    ("Wire", "连线"),
    (
        "Wire · W · drag or click two points",
        "连线 · W · 拖动或点击两个端点",
    ),
    ("Wiring", "布线"),
    ("XNOR Gate", "同或门"),
    ("XOR Gate", "异或门"),
    (
        "Your choice applies now. Retry to save it for the next launch.",
        "当前语言已生效。请重试保存，以便下次启动时继续使用。",
    ),
    ("Zero", "补零"),
    ("Zoom in", "放大"),
    ("Zoom out", "缩小"),
    ("settled", "已稳定"),
    ("{} bit", "{} 位"),
    ("{} bit: {}", "{} 位：{}"),
    ("{} components · {} wires", "{} 个元件 · {} 条导线"),
    (
        "{} interface pins · edit Pin components in the circuit definition",
        "{} 个接口引脚 · 可在电路定义中编辑引脚元件",
    ),
    ("{} pending", "{} 项待传播"),
    ("{} propagation steps", "已传播 {} 步"),
    ("{} shape(s) · {} port(s)", "{} 个图形 · {} 个端口"),
    ("{} · {} bit(s)", "{} · {} 位"),
    ("{} · {} · {} bit", "{} · {} · {} 位"),
    (
        "{} · {} · {} bit(s) · Pin ({}, {})",
        "{} · {} · {} 位 · 引脚（{}, {}）",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_is_complete_for_its_keys_and_keeps_format_arguments_opaque() {
        assert!(MESSAGES.windows(2).all(|w| w[0].0 < w[1].0));
        for &(en, zh) in MESSAGES {
            assert!(!zh.is_empty(), "{en}");
            assert_eq!(en.matches("{}").count(), zh.matches("{}").count(), "{en}");
            assert_eq!(with_language(Language::English, || tr(en).to_owned()), en);
            assert_eq!(
                with_language(Language::SimplifiedChinese, || tr(en).to_owned()),
                zh
            );
        }
        with_language(Language::SimplifiedChinese, || {
            assert_eq!(tf!("Circuit: {}", "File {} 电路"), "电路：File {} 电路");
            assert_eq!(
                tf!("{} components · {} wires", 3, 12),
                "3 个元件 · 12 条导线"
            );
            assert_eq!(
                tr("An unknown extension label"),
                "An unknown extension label"
            );
        });
    }
    #[test]
    fn language_scopes_restore_after_nesting_panics_and_threads() {
        with_language(Language::SimplifiedChinese, || {
            assert_eq!(tr("File"), "文件");
            with_language(Language::English, || assert_eq!(tr("File"), "File"));
            assert_eq!(tr("File"), "文件");
            std::thread::spawn(|| assert_eq!(tr("File"), "File"))
                .join()
                .unwrap();
        });
        let _ = std::panic::catch_unwind(|| {
            with_language(Language::SimplifiedChinese, || panic!("scope test"))
        });
        assert_eq!(tr("File"), "File");
    }
}
