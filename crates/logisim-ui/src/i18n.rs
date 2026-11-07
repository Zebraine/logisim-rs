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
    ("7-Segment Display", "七段数码管"),
    ("7-Segment Display Decoder", "七段译码器"),
    ("8-bit color channels", "8 位颜色通道"),
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
    ("Above / below", "上方 / 下方"),
    ("Action on overflow", "溢出动作"),
    ("Active on high", "高电平点亮"),
    ("Active-low preset / clear", "置位 / 清零低电平有效"),
    ("Actual size · 100%", "实际大小 · 100%"),
    ("Add", "添加"),
    ("Add Circuit… · Ctrl+Shift+N", "添加电路… · Ctrl+Shift+N"),
    ("Add Vertex", "添加顶点"),
    ("Add circuit", "添加电路"),
    ("Add vertex", "添加顶点"),
    ("Addend", "附加加数"),
    ("Adder", "加法器"),
    ("Address", "地址"),
    ("Address (hex)", "地址（十六进制）"),
    ("Address bits", "地址位宽"),
    ("Anchor", "锚点"),
    ("Appearance", "外观"),
    ("Appearance: {}", "外观：{}"),
    ("Apply", "应用"),
    (
        "Apply the hexadecimal value · Enter",
        "应用十六进制数值 · Enter",
    ),
    ("Arithmetic", "算术"),
    ("Arithmetic right", "算术右移"),
    ("Ascending", "升序"),
    (
        "At most 128 circuits are supported.",
        "最多支持 128 个电路。",
    ),
    ("Auto ticks", "自动时钟"),
    ("Automatic ticks · Ctrl+K", "自动时钟步进 · Ctrl+K"),
    ("Back", "返回"),
    ("Background", "背景"),
    ("Base", "基础"),
    ("Baseline", "基线"),
    ("Behavior", "行为"),
    ("Below / above", "下方 / 上方"),
    ("Bidirectional data", "双向数据"),
    ("Binary", "二进制"),
    ("Bit Adder", "位加法器"),
    ("Bit Extender", "位扩展器"),
    ("Bit Finder", "位查找器"),
    ("Bit Selector", "位选择器"),
    ("Bit count", "高电平位总数"),
    ("Bit index", "位索引"),
    ("Bit {}", "第 {} 位"),
    ("Blanking (BI, active low)", "消隐（BI，低电平有效）"),
    ("Blue", "蓝色"),
    ("Body color", "外壳颜色"),
    ("Body width ", "主体宽度 "),
    ("Bold", "粗体"),
    ("Borrow in", "借位输入"),
    ("Borrow out", "借位输出"),
    ("Bottom", "底部"),
    ("Bottom / left", "下侧 / 左侧"),
    ("Bottom segment (d)", "底段（d）"),
    ("Branch {}", "分支 {}"),
    ("Bring to Front", "置于顶层"),
    ("Buffer", "缓冲器"),
    ("Bus input", "总线输入"),
    ("Button", "按钮"),
    ("Button color", "按钮颜色"),
    ("Cancel", "取消"),
    (
        "Cannot remove the last circuit or a circuit that is still used by an instance.",
        "无法删除最后一个电路或仍被实例引用的电路。",
    ),
    (
        "Cannot save through a dangling symlink.",
        "无法通过失效的符号链接保存。",
    ),
    ("Carry in", "进位输入"),
    ("Carry out", "进位输出"),
    (
        "Carry: count is at the directional limit",
        "进位：计数已到当前方向的边界",
    ),
    ("Center", "居中"),
    ("Centered", "居中"),
    (
        "Changes update every instance. Wires keep their positions; reconnect moved ports in the circuit layout.",
        "修改会更新所有实例。导线保留原位置；端口移动后请在电路布局中重新接线。",
    ),
    (
        "Changing the input count keeps wire positions; reconnect moved inputs.",
        "修改输入数量会保留导线位置，请重新连接移动后的输入端口。",
    ),
    (
        "Changing the input format keeps wire positions; reconnect data inputs.",
        "切换输入形式会保留导线位置，请重新连接数据输入端口。",
    ),
    (
        "Chinese characters require a system CJK font (for example Noto Sans CJK SC).",
        "中文显示需要系统中的中文字体（例如 Noto Sans CJK SC）。",
    ),
    ("Chip select", "片选"),
    ("Chip select active on", "片选有效电平"),
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
    ("Circuit layout", "电路布局"),
    ("Circuit name", "电路名称"),
    ("Circuit operation unavailable", "无法执行电路操作"),
    ("Circuit: {}", "电路：{}"),
    ("Circular", "圆形"),
    ("Clear contents", "清空内容"),
    ("Clear: reset to zero", "清零：异步置零"),
    (
        "Click or drag a wire to branch. Ctrl-click selects; Ctrl-drag moves.",
        "点击或拖动导线以引出支路；Ctrl 单击选择，Ctrl 拖动移动。",
    ),
    (
        "Click or drag the wire to branch. Ctrl-drag moves; drag blue endpoint handles to reshape.",
        "点击或拖动导线以引出支路；Ctrl 拖动移动，拖动蓝色端点调整路径。",
    ),
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
    ("Column data", "列数据"),
    ("Columns", "按列"),
    (
        "Combinational and sequential simulation is available. Open and save the supported .circ components.",
        "支持组合与时序逻辑仿真，以及已实现元件的 .circ 读写。",
    ),
    ("Comparator", "比较器"),
    (
        "Component editing, wiring and canvas navigation are available.",
        "支持元件编辑、连线与画布导航。",
    ),
    ("Connected wires: {}", "相连导线：{}"),
    ("Constant", "常量"),
    ("Continue counting", "继续计数"),
    ("Control", "控制点"),
    ("Copy · Ctrl+C", "复制 · Ctrl+C"),
    ("Corner radius ", "圆角半径 "),
    (
        "Could not create a temporary save file",
        "无法创建临时保存文件",
    ),
    (
        "Count: 0 holds or loads; otherwise counts",
        "计数：0 时保持或加载，其余值允许计数",
    ),
    ("Counter", "计数器"),
    ("Curve", "曲线"),
    ("Custom appearance", "自定义外观"),
    ("Cut · Ctrl+X", "剪切 · Ctrl+X"),
    ("D Flip-Flop", "D 触发器"),
    ("Data bits", "数据位宽"),
    ("Data input", "数据输入"),
    ("Data interface", "数据接口"),
    ("Decimal point", "小数点"),
    ("Decoder", "译码器"),
    ("Default appearance", "默认外观"),
    ("Delete", "删除"),
    ("Demultiplexer", "解复用器"),
    ("Descending", "降序"),
    ("Dip Switch", "拨码开关"),
    ("Direction", "方向"),
    ("Disabled output", "禁用时输出"),
    ("Discard changes", "放弃修改"),
    ("Distribute", "分配"),
    ("Dividend high", "被除数高位"),
    ("Dividend low", "被除数低位"),
    ("Divider", "除法器"),
    ("Divisor", "除数"),
    ("Dot shape", "光点形状"),
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
    ("Edit contents…", "编辑内容…"),
    ("Ellipse", "椭圆"),
    ("Enable", "使能"),
    ("Enable (0 holds outputs)", "使能（0 时保持输出）"),
    (
        "Enable / pause simulation · Ctrl+E",
        "启用 / 暂停仿真 · Ctrl+E",
    ),
    ("Enable Simulation · Ctrl+E", "启用仿真 · Ctrl+E"),
    ("Enable output", "级联使能输出"),
    ("Enable: 0 holds data", "写使能：0 时保持数据"),
    ("Enable: 0 stops counting", "使能：0 时停止计数"),
    ("End", "终点"),
    ("End: {}, {}", "终点：{}, {}"),
    ("Enter a file name.", "请输入文件名。"),
    (
        "Enter a hexadecimal value within {} bits.",
        "请输入 {} 位范围内的十六进制数值。",
    ),
    ("Equal", "相等"),
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
    ("Falling edge", "下降沿"),
    ("Fan out", "分支数量"),
    ("File", "文件"),
    ("File exceeds the 8 MiB limit.", "文件超过 8 MiB 限制。"),
    ("File is not UTF-8: {}", "文件不是 UTF-8 编码：{}"),
    ("File name or full path", "文件名或完整路径"),
    ("File operation failed", "文件操作失败"),
    ("Fill", "填充"),
    ("Fill range", "填充范围"),
    ("First corner", "第一个角"),
    ("Floating", "悬空"),
    ("Font family", "字体"),
    ("Font size", "字号"),
    ("Found", "找到目标位"),
    ("Front", "置于顶层"),
    ("Gate size", "门尺寸"),
    ("Gates", "逻辑门"),
    ("Generate package", "生成封装"),
    ("Greater than", "大于"),
    ("Green", "绿色"),
    ("Grid", "网格"),
    ("Ground", "接地"),
    ("Group index", "分组索引"),
    ("Group signal", "有效输入指示"),
    ("Help", "帮助"),
    ("Hex Digit Display", "十六进制显示器"),
    ("Hexadecimal", "十六进制"),
    ("Hexadecimal input", "十六进制输入"),
    (
        "Hierarchy exceeds 32 levels or 1024 instances.",
        "层级超过 32 层或 1024 个实例。",
    ),
    ("High duration (ticks)", "高电平持续步数"),
    ("High level", "高电平"),
    ("Highest-order 0", "最高位的 0"),
    ("Highest-order 1", "最高位的 1"),
    ("Hold to press", "按住按钮"),
    ("Horizontal", "水平对齐"),
    ("Imported supported content only", "仅导入已支持的内容"),
    ("Include enable", "显示使能端口"),
    ("Include pin names", "包含引脚名称"),
    ("Incompatible bit widths", "位宽不兼容"),
    ("Index bits", "索引位宽"),
    ("Input", "输入"),
    ("Input (hex)", "输入（十六进制）"),
    ("Input bits", "输入位宽"),
    ("Input format", "输入格式"),
    ("Input {}", "输入 {}"),
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
    ("J-K Flip-Flop", "JK 触发器"),
    ("LED", "LED"),
    ("LED Matrix", "LED 点阵"),
    ("Label", "标签"),
    ("Label location", "标签位置"),
    ("Lamp test (LT, active low)", "灯测试（LT，低电平有效）"),
    ("Language", "语言"),
    (
        "Language changes apply immediately and are saved for the next launch.",
        "语言切换立即生效，并在下次启动时保留。",
    ),
    ("Left", "左"),
    ("Left to right", "从左到右"),
    ("Left-handed", "左侧"),
    ("Legacy", "传统"),
    ("Less than", "小于"),
    ("Light persistence (ticks)", "余辉（Tick 数）"),
    ("Line", "线段"),
    ("Load Library…", "加载元件库…"),
    ("Load next value", "加载下一值"),
    (
        "Load: 0 writes; otherwise reads",
        "加载：0 写入，其他值读取",
    ),
    ("Load: 1 loads data", "加载：1 时加载数据"),
    (
        "Load: 1 loads when Count=0, otherwise decrements",
        "加载：1 且计数为 0 时加载，否则递减",
    ),
    ("Logical left", "逻辑左移"),
    ("Logical right", "逻辑右移"),
    (
        "Logisim-ITA compatible .circ · supported components only",
        "兼容 Logisim-ITA 的 .circ · 仅限已支持元件",
    ),
    ("Low duration (ticks)", "低电平持续步数"),
    ("Low level", "低电平"),
    ("Lower", "下移一层"),
    ("Lower · Ctrl+Down", "下移一层 · Ctrl+Down"),
    ("Lower-left segment (e)", "左下段（e）"),
    ("Lower-right segment (c)", "右下段（c）"),
    ("Lowest-order 0", "最低位的 0"),
    ("Lowest-order 1", "最低位的 1"),
    ("Matrix columns", "点阵列数"),
    ("Matrix rows", "点阵行数"),
    ("Maximize / Restore", "最大化 / 还原"),
    ("Maximum value (hex)", "最大值（十六进制）"),
    ("Medium", "中"),
    ("Memory", "存储"),
    ("Middle segment (g)", "中段（g）"),
    ("Minimize", "最小化"),
    ("Missing circuit", "缺少电路"),
    ("Missing file name", "缺少文件名"),
    ("Missing parent directory", "缺少父文件夹"),
    ("Missing simulation root.", "缺少仿真根电路。"),
    (
        "Mode: 0 holds; 1 shifts toward bit 0; 2 shifts toward the high bit; 3 loads data",
        "模式：0 保持；1 向低位移；2 向高位移；3 并行加载",
    ),
    ("Move Circuit Down", "下移电路"),
    ("Move Circuit Up", "上移电路"),
    ("Move down", "下移"),
    (
        "Move this port to arrange the package. Edit or remove its Pin in the circuit layout.",
        "移动此端口以调整封装。在电路布局中编辑或删除对应引脚。",
    ),
    ("Move up", "上移"),
    ("Multiplexer", "多路选择器"),
    ("Multiplier", "乘法器"),
    ("NAND Gate", "与非门"),
    ("NOR Gate", "或非门"),
    ("NOT Gate", "非门"),
    ("Narrow", "窄"),
    ("Negator", "取负器"),
    ("New behavior", "新行为"),
    ("New layout", "新布局"),
    ("New · Ctrl+N", "新建 · Ctrl+N"),
    ("Next page", "下一页"),
    ("No input state", "无输入时输出"),
    ("No tools available yet", "暂无可用元件"),
    ("None", "无"),
    ("North", "北"),
    ("Number of stages", "级数"),
    ("Number of switches", "开关数量"),
    ("Numeric type", "数值类型"),
    ("OK", "确定"),
    ("OR Gate", "或门"),
    ("Octal", "八进制"),
    ("Off color", "熄灭颜色"),
    ("Old behavior", "旧行为"),
    ("On color", "点亮颜色"),
    ("One", "补一"),
    ("Open", "打开"),
    ("Open circuit", "打开电路"),
    ("Open… · Ctrl+O", "打开… · Ctrl+O"),
    ("Operand A", "操作数 A"),
    ("Operand B", "操作数 B"),
    ("Opposite corner", "对角"),
    ("Outline", "描边"),
    ("Output", "输出"),
    ("Output bits", "输出位宽"),
    ("Output pin", "输出引脚"),
    ("Output {}", "输出 {}"),
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
    (
        "Poke: click each switch to toggle its output.",
        "交互工具：点击每个拨码开关以切换输出。",
    ),
    (
        "Poke: click to open or close the switch.",
        "交互工具：点击开关以接通或断开。",
    ),
    (
        "Poke: drag the handle. The value is saved with the circuit.",
        "交互工具：拖动滑块手柄，数值随电路保存。",
    ),
    (
        "Poke: hold the left mouse button to press; release anywhere.",
        "交互工具：按住鼠标左键按下按钮，在任意位置松开即可释放。",
    ),
    ("Polygon", "多边形"),
    ("Polyline", "折线"),
    ("Port", "端口"),
    ("Port spacing ", "端口间距 "),
    ("Ports", "端口"),
    ("Power", "电源"),
    ("Preset / clear positions", "置位 / 清零位置"),
    ("Preset: set all bits", "置位：异步置为全 1"),
    ("Previous page", "上一页"),
    ("Print…", "打印…"),
    ("Priority Encoder", "优先编码器"),
    ("Probe", "探针"),
    ("Product high", "乘积高位"),
    ("Product low", "乘积低位"),
    ("Project", "项目"),
    ("Project Options…", "项目选项…"),
    ("Propagate one batch · Ctrl+I", "传播一个批次 · Ctrl+I"),
    ("Propagation limit — paused", "达到传播上限 — 已暂停"),
    (
        "Propagation limit: check feedback, then Reset or Run",
        "达到传播上限：请检查反馈，再重置或运行",
    ),
    ("Pull Resistor", "上拉 / 下拉电阻"),
    (
        "Queue one clock tick · Ctrl+T; paused simulation uses Step or Run",
        "时钟步进 · Ctrl+T；暂停时使用单步或运行来传播",
    ),
    ("Quotient", "商"),
    ("RAM", "RAM"),
    ("RAM contents", "RAM 内容"),
    (
        "RAM contents belong to this simulation instance. Reset clears them; circuit files do not save them.",
        "RAM 内容属于当前仿真实例，复位会清空，电路文件不保存这些内容。",
    ),
    ("RGB LED", "RGB 发光二极管"),
    ("ROM", "ROM"),
    ("ROM contents", "ROM 内容"),
    (
        "ROM contents are saved with the circuit. Edits support Undo.",
        "ROM 内容随电路保存，编辑支持撤销。",
    ),
    ("Radix", "进制"),
    ("Raise", "上移一层"),
    ("Raise · Ctrl+Up", "上移一层 · Ctrl+Up"),
    ("Rect", "矩形"),
    ("Rectangle", "矩形"),
    ("Red", "红色"),
    ("Redo · Ctrl+Shift+Z", "重做 · Ctrl+Shift+Z"),
    ("Refresh", "刷新"),
    ("Register", "寄存器"),
    ("Remainder", "余数"),
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
    ("Reset canvas", "重置画布"),
    ("Reset canvas · Ctrl+0", "重置画布 · Ctrl+0"),
    (
        "Reset inputs, storage and clocks · Ctrl+R",
        "重置输入、存储状态和时钟 · Ctrl+R",
    ),
    ("Result", "结果"),
    ("Retry", "重试"),
    ("Return to Parent · Alt+Left", "返回父电路 · Alt+Left"),
    (
        "Return to parent instance · Alt+Left",
        "返回父实例 · Alt+Left",
    ),
    ("Reverse: 1 counts down", "反向：1 时递减"),
    ("Revert to Default Appearance", "恢复默认外观"),
    ("Revert to default appearance", "恢复默认外观"),
    ("Right", "右"),
    ("Right to left", "从右到左"),
    ("Right-handed", "右侧"),
    ("Rising edge", "上升沿"),
    ("Rotate", "旋转"),
    ("Rotate clockwise · R", "顺时针旋转 · R"),
    ("Rotate left", "循环左移"),
    ("Rotate right", "循环右移"),
    ("Rotate · R", "旋转 · R"),
    ("Round", "圆角"),
    ("Rounded rectangle", "圆角矩形"),
    ("Row data", "行数据"),
    ("Row selection", "行选择"),
    ("Rows", "按行"),
    ("Run", "运行"),
    ("Running", "运行中"),
    ("S-R Flip-Flop", "SR 触发器"),
    ("Save", "保存"),
    ("Save As… · Ctrl+Shift+S", "另存为… · Ctrl+Shift+S"),
    ("Save changes?", "保存修改？"),
    ("Save circuit as", "电路另存为"),
    ("Save · Ctrl+S", "保存 · Ctrl+S"),
    ("Search type", "查找类型"),
    ("Segment {}", "段 {}"),
    ("Select", "选择"),
    ("Select All · Ctrl+A", "全选 · Ctrl+A"),
    (
        "Select affected components and wires",
        "选择受影响的元件和导线",
    ),
    ("Select bits", "选择位数"),
    ("Select location", "选择端口位置"),
    ("Select rows/columns", "行列选择"),
    ("Select · V", "选择 · V"),
    (
        "Select, drag or use arrow keys to move. Drag shape handles to reshape. Ports and anchor stay on the circuit grid.",
        "选择后拖动或使用方向键移动，拖动控制点调整图形。端口与锚点始终吸附到电路网格。",
    ),
    ("Selection: {}", "选择：{}"),
    ("Selection: {} item(s)", "已选择 {} 项"),
    ("Send to Back", "置于底层"),
    ("Separate read and write ports", "独立读写端口"),
    ("Set as Main Circuit", "设为主电路"),
    ("Settings", "设置"),
    ("Settings need attention.", "设置需要处理。"),
    ("Shared data port · asynchronous", "共享数据端口 · 异步"),
    ("Shared data port · synchronous", "共享数据端口 · 同步"),
    ("Shift Register", "移位寄存器"),
    ("Shift amount", "移位量"),
    ("Shift amount bits", "移位量位宽"),
    ("Shift type", "移位类型"),
    ("Shifter", "移位器"),
    ("Show grid", "显示网格"),
    ("Showing the first 32 issues.", "仅显示前 32 个问题。"),
    ("Sign", "符号位"),
    ("Signal: {}", "信号：{}"),
    ("Signed decimal", "有符号十进制"),
    ("Simplified mode", "简化模式"),
    ("Simulate", "仿真"),
    ("Simulation issues ({})", "仿真问题（{}）"),
    ("Size ", "字号 "),
    ("Slider", "滑块"),
    ("Snap", "网格吸附"),
    ("South", "南"),
    ("Splitter", "分线器"),
    ("Square", "方形"),
    ("Start", "起点"),
    ("Start: {}, {}", "起点：{}, {}"),
    ("Stay at value", "保持边界值"),
    ("Step", "单步"),
    ("Step Simulation · Ctrl+I", "单步仿真 · Ctrl+I"),
    ("Store: 0 holds data", "存储：0 保持数据"),
    ("Stored value: {}", "存储值：{}"),
    ("Subcircuit", "子电路"),
    ("Subtractor", "减法器"),
    ("Switch", "开关"),
    ("T Flip-Flop", "T 触发器"),
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
    ("Three-state outputs", "三态输出"),
    ("Through address (hex)", "结束地址（十六进制）"),
    ("Tick", "时钟步进"),
    ("Tick frequency", "时钟步进频率"),
    ("Tick once · Ctrl+T", "时钟步进 · Ctrl+T"),
    ("Toggle bit {}", "切换第 {} 位"),
    ("Toggle clock output", "翻转时钟输出"),
    ("Toggle stored bit", "翻转存储位"),
    ("Tool: {}", "工具：{}"),
    ("Top", "顶部"),
    ("Top / right", "上侧 / 右侧"),
    ("Top segment (a)", "顶段（a）"),
    ("Transistor", "晶体管"),
    ("Transmission Gate", "传输门"),
    ("Trigger", "触发方式"),
    ("Tunnel", "隧道"),
    ("Tunnel · U", "隧道 · U"),
    ("Two's complement", "补码"),
    ("Undo · Ctrl+Z", "撤销 · Ctrl+Z"),
    (
        "Unknown or invalid language preference.",
        "语言配置未知或无效。",
    ),
    ("Unsigned", "无符号"),
    ("Unsigned decimal", "无符号十进制"),
    ("Untitled", "未命名"),
    ("Up", "上级目录"),
    ("Upper-left segment (f)", "左上段（f）"),
    ("Upper-right segment (b)", "右上段（b）"),
    (
        "Use File to open or save the supported .circ subset.",
        "使用文件菜单打开或保存支持的 .circ 文件。",
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
    ("Wrap around", "回绕"),
    ("Write word", "写入单字"),
    ("XNOR Gate", "同或门"),
    ("XOR Gate", "异或门"),
    (
        "Your choice applies now. Retry to save it for the next launch.",
        "当前语言已生效。请重试保存，以便下次启动时继续使用。",
    ),
    ("Zero", "补零"),
    (
        "Zero blanking (RBI, active low)",
        "零值消隐（RBI，低电平有效）",
    ),
    ("Zero output", "零"),
    ("Zoom in", "放大"),
    ("Zoom out", "缩小"),
    ("settled", "已稳定"),
    ("{} bit", "{} 位"),
    ("{} bit: {}", "{} 位：{}"),
    ("{} clock ticks · {} ticks/s", "时钟已步进 {} 次 · {} 步/秒"),
    ("{} components · {} wires", "{} 个元件 · {} 条导线"),
    (
        "{} interface pins · edit Pin components in the circuit definition",
        "{} 个接口引脚 · 可在电路定义中编辑引脚元件",
    ),
    ("{} pending", "{} 项待传播"),
    ("{} propagation steps", "已传播 {} 步"),
    ("{} shape(s) · {} port(s)", "{} 个图形 · {} 个端口"),
    ("{} ticks/s", "{} 步/秒"),
    ("{} words · {} bits per word", "{} 个字 · 每字 {} 位"),
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
