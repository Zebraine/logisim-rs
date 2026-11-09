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
pub(crate) fn current_language() -> Language {
    LANGUAGE.get()
}
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
    ("0/1", "0/1"),
    ("0/floating", "0/浮空"),
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
    ("AND terms", "AND 项数"),
    ("ASCII (7-bit)", "ASCII（7 位）"),
    ("About logisim-rs", "关于 logisim-rs"),
    ("Above / below", "上方 / 下方"),
    ("Action on overflow", "溢出动作"),
    ("Active on high", "高电平点亮"),
    ("Active-low preset / clear", "置位 / 清零低电平有效"),
    ("Actual plugin location", "插件实际位置"),
    ("Actual size · 100%", "实际大小 · 100%"),
    ("Actual: {} Hz", "实际：{} Hz"),
    ("Add", "添加"),
    ("Add Circuit… · Ctrl+Shift+N", "添加电路… · Ctrl+Shift+N"),
    ("Add Vertex", "添加顶点"),
    (
        "Add at least one output before building a circuit.",
        "生成电路前请至少添加一个输出。",
    ),
    ("Add circuit", "添加电路"),
    ("Add vertex", "添加顶点"),
    ("Addend", "附加加数"),
    ("Adder", "加法器"),
    ("Address", "地址"),
    ("Address (hex)", "地址（十六进制）"),
    ("Address bits", "地址位宽"),
    (
        "An output is unknown or erroneous; analysis requires 0 or 1 for every input row:",
        "输出存在未知值或错误；分析要求每一行输出均为 0 或 1：",
    ),
    (
        "Analysis requires at least one output Pin.",
        "分析需要至少一个输出引脚。",
    ),
    (
        "Analysis requires single-bit input and output Pins.",
        "分析仅支持单比特输入和输出引脚。",
    ),
    ("Analyze Circuit", "分析电路"),
    ("Analyze Circuit…", "分析电路…"),
    ("Analyzing circuit…", "正在分析电路…"),
    ("Anchor", "锚点"),
    ("Appearance", "外观"),
    ("Appearance: {}", "外观：{}"),
    ("Applied: {}", "已应用：{}"),
    ("Apply", "应用"),
    (
        "Apply or revert the expression before continuing.",
        "请先应用或还原表达式，再继续操作。",
    ),
    (
        "Apply the hexadecimal value · Enter",
        "应用十六进制数值 · Enter",
    ),
    ("Arithmetic", "算术"),
    ("Arithmetic right", "算术右移"),
    ("Ascending", "升序"),
    (
        "At most 12 inputs are supported. Switch to expression mode for more inputs.",
        "最多支持 12 个输入，更多输入请切换到表达式模式。",
    ),
    (
        "At most 128 circuits are supported.",
        "最多支持 128 个电路。",
    ),
    (
        "At most 32 expression inputs are supported.",
        "表达式模式最多支持 32 个输入。",
    ),
    ("At most 32 outputs are supported.", "最多支持 32 个输出。"),
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
    ("Bottom/Right", "下/右"),
    ("Branch {}", "分支 {}"),
    ("Bring to Front", "置于顶层"),
    ("Browse…", "浏览…"),
    ("Buffer", "缓冲器"),
    ("Buffer length", "缓冲区长度"),
    ("Build Circuit", "生成电路"),
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
        "Changes apply without resetting the circuit and are saved for the next launch. Manual propagation steps always use events.",
        "切换不会复位电路，并会记住设置。手动传播单步始终使用事件传播。",
    ),
    (
        "Changes update every instance. Wires keep their positions; reconnect moved ports in the circuit layout.",
        "修改会更新所有实例。导线保留原位置；端口移动后请在电路布局中重新接线。",
    ),
    (
        "Changing TTL facing or power pins keeps wire positions; reconnect moved ports.",
        "修改 TTL 朝向或电源引脚会保留导线位置；请重新连接移动后的端口。",
    ),
    (
        "Changing control location or gate size keeps wire positions; reconnect moved ports.",
        "修改控制端位置或门尺寸会保留导线位置，请重新连接移动后的端口。",
    ),
    (
        "Changing gate location or facing keeps wire positions; reconnect moved ports.",
        "更改栅极位置或朝向会保留导线位置，请重新连接移动的端口。",
    ),
    (
        "Changing the input count keeps wire positions; reconnect moved inputs.",
        "修改输入数量会保留导线位置，请重新连接移动后的输入端口。",
    ),
    (
        "Changing the input format keeps wire positions; reconnect data inputs.",
        "切换输入形式会保留导线位置，请重新连接数据输入端口。",
    ),
    ("Character", "字符"),
    ("Character available", "有字符可读"),
    ("Character encoding", "字符编码"),
    ("Check", "检查"),
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
    ("Choose a regular .hex file.", "请选择普通 .hex 文件。"),
    ("Choose a regular .wasm file.", "请选择普通 .wasm 文件。"),
    (
        "Choose missing plugin locations. Confirm can continue with static graphics for unresolved components.",
        "请选择缺失插件的位置。可直接确认，未解决的元件将仅显示静态图形。",
    ),
    ("Circuit analysis cancelled.", "电路分析已取消。"),
    (
        "Circuit analysis cannot verify that a plugin is stateless:",
        "电路分析无法确认插件无状态：",
    ),
    (
        "Circuit analysis could not evaluate this circuit.",
        "无法求值此电路。",
    ),
    (
        "Circuit analysis does not support feedback connections.",
        "电路分析不支持反馈连接。",
    ),
    (
        "Circuit analysis does not support interactive inputs; use input Pins:",
        "电路分析不支持交互输入，请使用输入引脚：",
    ),
    (
        "Circuit analysis does not support this component:",
        "电路分析尚不支持此元件：",
    ),
    (
        "Circuit analysis does not support timing or stateful components:",
        "电路分析不支持时序或有状态的元件：",
    ),
    (
        "Circuit analysis exceeded the settling limit.",
        "电路分析超出稳定态收敛上限。",
    ),
    (
        "Circuit analysis exceeds the hierarchy limit.",
        "电路分析超出层级或实例数量上限。",
    ),
    (
        "Circuit analysis exceeds the item limit.",
        "电路分析超出元件和导线数量上限。",
    ),
    (
        "Circuit analysis found an electrical error at input row:",
        "电路分析发现电气错误，输入行：",
    ),
    (
        "Circuit analysis found incompatible signal widths.",
        "电路分析发现不兼容的信号位宽。",
    ),
    (
        "Circuit analysis is unavailable for an import with omitted content.",
        "导入时省略了部分内容，无法分析该电路。",
    ),
    (
        "Circuit analysis requires complete, valid subcircuit interfaces:",
        "电路分析需要完整、有效的子电路接口：",
    ),
    ("Circuit analysis worker stopped.", "电路分析线程已停止。"),
    ("Circuit label", "封装标签"),
    ("Circuit layout", "电路布局"),
    ("Circuit name", "电路名称"),
    ("Circuit operation unavailable", "无法执行电路操作"),
    ("Circuit: {}", "电路：{}"),
    ("Circular", "圆形"),
    ("Clear", "清空"),
    ("Clear contents", "清空内容"),
    ("Clear: reset to zero", "清零：异步置零"),
    ("Clear: restart from seed", "复位：从种子重新开始"),
    (
        "Click an output: 0 → 1 → x. x is a don't-care and will become 0 or 1 in the generated circuit.",
        "点击输出切换：0 → 1 → x。x 为无关项，生成电路时将确定为 0 或 1。",
    ),
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
        "Columns run from high bit to low bit. Empty terms or outputs evaluate to E.",
        "列从高位排到低位。空项或无连接的输出为 E。",
    ),
    ("Combinational Circuit Generator", "组合电路生成器"),
    (
        "Combinational and sequential simulation is available. Open and save the supported .circ components.",
        "支持组合与时序逻辑仿真，以及已实现元件的 .circ 读写。",
    ),
    ("Comparator", "比较器"),
    (
        "Complete truth tables support at most 12 inputs. The expressions are preserved.",
        "完整真值表最多支持 12 个输入，表达式已保留。",
    ),
    (
        "Completed UI frames per second, measured over about one second. Idle redraws are included.",
        "按约一秒内实际完成的界面帧数统计，包含闲置时的刷新。",
    ),
    (
        "Component editing, wiring and canvas navigation are available.",
        "支持元件编辑、连线与画布导航。",
    ),
    ("Computation cancelled.", "计算已取消。"),
    ("Computation worker stopped.", "计算线程已停止。"),
    ("Computing…", "正在计算…"),
    (
        "Configured automatic clock tick frequency.",
        "设定的自动时钟步进频率。",
    ),
    ("Confirm", "确认"),
    ("Connected wires: {}", "相连导线：{}"),
    ("Constant", "常量"),
    ("Continue counting", "继续计数"),
    ("Control", "控制点"),
    ("Control location", "控制端位置"),
    ("Controlled Buffer", "受控缓冲器"),
    ("Controlled Inverter", "受控反相器"),
    (
        "Convert all logic gates, including XOR, to NAND gates. Pins and constants are retained.",
        "将所有逻辑门（含异或）转换为 NAND 门，保留引脚与常量。",
    ),
    ("Copy · Ctrl+C", "复制 · Ctrl+C"),
    ("Corner radius ", "圆角半径 "),
    (
        "Could not create a temporary save file",
        "无法创建临时保存文件",
    ),
    ("Could not create circuit:", "无法创建电路："),
    ("Could not start circuit analysis:", "无法启动电路分析："),
    ("Could not start computation:", "无法开始计算："),
    (
        "Count: 0 holds or loads; otherwise counts",
        "计数：0 时保持或加载，其余值允许计数",
    ),
    ("Counter", "计数器"),
    ("Create Truth Table", "生成真值表"),
    (
        "Create a truth table before minimizing.",
        "化简前请先生成真值表。",
    ),
    ("Curve", "曲线"),
    ("Custom appearance", "自定义外观"),
    ("Custom tick frequency", "自定义时钟频率"),
    ("Custom…", "自定义…"),
    ("Cut · Ctrl+X", "剪切 · Ctrl+X"),
    ("D Flip-Flop", "D 触发器"),
    ("D Latch", "D 锁存器"),
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
    ("Display columns", "显示列数"),
    ("Display rows", "显示行数"),
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
    ("Drag to move; release to center.", "拖动摇杆，松开回中。"),
    ("Drawing style", "绘图样式"),
    ("Duplicate", "重复"),
    ("Duplicate variable name:", "变量名称重复："),
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
    ("Edit programming table…", "编辑编程表…"),
    ("Ellipse", "椭圆"),
    ("Embedded library snapshot", "已嵌入的库快照"),
    ("Enable", "使能"),
    ("Enable (0 holds outputs)", "使能（0 时保持输出）"),
    ("Enable (active high)", "使能（高电平有效）"),
    ("Enable (active low)", "使能（低电平有效）"),
    (
        "Enable / pause simulation · Ctrl+E",
        "启用 / 暂停仿真 · Ctrl+E",
    ),
    ("Enable Simulation · Ctrl+E", "启用仿真 · Ctrl+E"),
    ("Enable output", "级联使能输出"),
    ("Enable truth table generation", "启用真值表生成"),
    ("Enable: 0 holds data", "写使能：0 时保持数据"),
    ("Enable: 0 stops counting", "使能：0 时停止计数"),
    ("End", "终点"),
    ("End: {}, {}", "终点：{}, {}"),
    ("Enter a file name.", "请输入文件名。"),
    (
        "Enter a hexadecimal value within {} bits.",
        "请输入 {} 位范围内的十六进制数值。",
    ),
    (
        "Enter a whole number from 1 to {}.",
        "请输入 1 至 {} 之间的整数。",
    ),
    ("Equal", "相等"),
    ("Error", "错误"),
    (
        "Error signal: conflicting drivers / undefined input",
        "错误信号：驱动冲突 / 未定义输入",
    ),
    ("Even Parity", "偶校验门"),
    ("Event propagation", "事件传播"),
    ("Exactly one", "恰好一个为 1"),
    ("Exit", "退出"),
    (
        "Expanded hierarchy exceeds 65536 items.",
        "展开后的层级超过 65536 项。",
    ),
    (
        "Expected an operand at character:",
        "缺少操作数，字符位置：",
    ),
    (
        "Expected closing parenthesis at character:",
        "缺少右括号，字符位置：",
    ),
    ("Export Image…", "导出图片…"),
    ("Expression", "表达式"),
    (
        "Expression exceeds the size or depth limit.",
        "表达式超出大小或嵌套深度限制。",
    ),
    ("Expression mode", "表达式模式"),
    (
        "Expressions are stored without enumerating inputs. Generate a complete table only when needed.",
        "表达式直接保存，不枚举输入；需要时再生成完整真值表。",
    ),
    ("Extension", "扩展方式"),
    ("FPS: {}", "帧率：{} FPS"),
    ("Facing", "朝向"),
    ("Falling edge", "下降沿"),
    ("Fan out", "分支数量"),
    ("File", "文件"),
    ("File exceeds the 8 MiB limit.", "文件超过 8 MiB 限制。"),
    ("File is not UTF-8", "文件不是 UTF-8 编码"),
    ("File is not UTF-8: {}", "文件不是 UTF-8 编码：{}"),
    ("File name or full path", "文件名或完整路径"),
    ("File operation failed", "文件操作失败"),
    ("Fill", "填充"),
    ("Fill range", "填充范围"),
    ("First corner", "第一个角"),
    ("Floating", "悬空"),
    ("Font family", "字体"),
    ("Font size", "字号"),
    ("Format", "形式"),
    ("Found", "找到目标位"),
    ("Frequency (Hz)", "频率（Hz）"),
    ("Front", "置于顶层"),
    ("Gate location", "栅极位置"),
    ("Gate size", "门尺寸"),
    ("Gates", "逻辑门"),
    ("Generate Circuit…", "生成电路…"),
    ("Generate package", "生成封装"),
    (
        "Generated circuit exceeds the 4096-item limit. Simplify the logic or split the outputs.",
        "生成电路超出 4096 项限制。请简化逻辑或拆分输出。",
    ),
    ("Greater than", "大于"),
    ("Green", "绿色"),
    ("Grid", "网格"),
    ("Ground", "接地"),
    ("Group index", "分组索引"),
    ("Group signal", "有效输入指示"),
    ("Help", "帮助"),
    ("Hex Digit Display", "十六进制显示器"),
    ("Hex inverter", "六路反相器"),
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
    ("Import .hex…", "导入 .hex…"),
    (
        "Import an editable snapshot; saved projects do not need the source library.",
        "导入可编辑快照；保存后的项目无需原库文件。",
    ),
    ("Import memory image", "导入存储器映像"),
    ("Imported supported content only", "仅导入已支持的内容"),
    ("Include Vcc/Gnd pins", "显示 Vcc/GND 引脚"),
    ("Include enable", "显示使能端口"),
    ("Include pin names", "包含引脚名称"),
    ("Incompatible bit widths", "位宽不兼容"),
    ("Index bits", "索引位宽"),
    ("Input", "输入"),
    ("Input (hex)", "输入（十六进制）"),
    ("Input bits", "输入位宽"),
    ("Input format", "输入格式"),
    ("Input variables", "输入变量"),
    ("Input {}", "输入 {}"),
    ("Input {} (inverted)", "输入 {}（取反）"),
    ("Input/Output", "输入 / 输出"),
    (
        "Input: — disconnected, 1 direct, 0 inverted. Click to cycle. Output: ● connected.",
        "输入：— 不连接，1 原值，0 反值。点击切换。输出：● 已连接。",
    ),
    ("Inputs", "输入数量"),
    (
        "Inputs have pending changes. Step or Run to propagate.",
        "输入有待传播的变化，请单步或运行。",
    ),
    ("Inputs: {} · Outputs: {}", "输入：{} · 输出：{}"),
    (
        "Instance inputs are driven by the parent circuit. Edits apply to all instances.",
        "实例输入由父电路驱动。编辑会应用到所有实例。",
    ),
    ("Instruction", "指令"),
    ("Internal Pin: ({}, {})", "内部引脚：（{}, {}）"),
    ("Invalid circuit project:", "电路项目无效："),
    ("Invalid library name.", "库名称无效。"),
    ("Invert input {}", "输入 {} 取反"),
    ("Italic", "斜体"),
    ("J-K Flip-Flop", "JK 触发器"),
    ("Joystick", "摇杆"),
    ("Keyboard", "键盘"),
    ("LED", "LED"),
    ("LED Matrix", "LED 点阵"),
    ("Label", "标签"),
    ("Label color", "标签颜色"),
    ("Label location", "标签位置"),
    ("Label up", "标签朝上方向"),
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
    (
        "Library import cancelled: unsupported content would be omitted.\n\n{}",
        "已取消库导入：以下未支持内容会被省略。\n\n{}",
    ),
    ("Light persistence (ticks)", "余辉（Tick 数）"),
    ("Line", "线段"),
    (
        "Line {}: expected a nonnegative decimal repeat count.",
        "第 {} 行：重复次数须为非负十进制整数。",
    ),
    (
        "Line {}: invalid hexadecimal word.",
        "第 {} 行：十六进制数据无效。",
    ),
    (
        "Line {}: the image exceeds this memory's address capacity.",
        "第 {} 行：映像超出此存储器的地址容量。",
    ),
    ("Load .circ Library…", "加载 .circ 库…"),
    ("Load Library…", "加载元件库…"),
    ("Load Wasm Plugin…", "加载 Wasm 插件…"),
    ("Load next value", "加载下一值"),
    (
        "Load the matching plugin, or Reset after correcting the problem.",
        "请加载对应插件，或在解决问题后重置仿真。",
    ),
    (
        "Load: 0 writes; otherwise reads",
        "加载：0 写入，其他值读取",
    ),
    ("Load: 1 loads data", "加载：1 时加载数据"),
    (
        "Load: 1 loads when Count=0, otherwise decrements",
        "加载：1 且计数为 0 时加载，否则递减",
    ),
    ("Locate circuit plugins", "定位电路插件"),
    (
        "Location changed; Check or Confirm to validate.",
        "位置已更改，请点击检查或确认进行验证。",
    ),
    ("Logical left", "逻辑左移"),
    ("Logical right", "逻辑右移"),
    (
        "Logisim .circ · Wasm components require logisim-rs",
        ".circ 电路 · Wasm 元件需要 logisim-rs",
    ),
    (
        "Logisim .hex · v2.0 raw · replaces all words",
        "Logisim .hex · v2.0 raw · 覆盖全部内容",
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
    ("Missing circuit to analyze.", "找不到待分析的电路。"),
    ("Missing file name", "缺少文件名"),
    ("Missing parent directory", "缺少父文件夹"),
    ("Missing simulation root.", "缺少仿真根电路。"),
    (
        "Mode: 0 holds; 1 shifts toward bit 0; 2 shifts toward the high bit; 3 loads data",
        "模式：0 保持；1 向低位移；2 向高位移；3 并行加载",
    ),
    ("Move Circuit Down", "下移电路"),
    ("Move Circuit Up", "上移电路"),
    ("Move Down", "下移"),
    ("Move Up", "上移"),
    ("Move down", "下移"),
    (
        "Move this port to arrange the package. Edit or remove its Pin in the circuit layout.",
        "移动此端口以调整封装。在电路布局中编辑或删除对应引脚。",
    ),
    ("Move up", "上移"),
    ("Multiplexer", "多路选择器"),
    ("Multiplier", "乘法器"),
    ("N control (active high)", "N 控制端（高电平有效）"),
    ("N-Type", "N 型"),
    ("NAND Gate", "与非门"),
    ("NOR Gate", "或非门"),
    ("NOT Gate", "非门"),
    (
        "NOT: ~A, !A, A' · AND: A & B, A B · XOR: A ^ B · OR: A | B, A + B",
        "非：~A、!A、A' · 与：A & B、A B · 异或：A ^ B · 或：A | B、A + B",
    ),
    ("Narrow", "窄"),
    ("Negate control input", "反转控制输入"),
    ("Negator", "取负器"),
    ("New behavior", "新行为"),
    ("New circuit name", "新电路名称"),
    ("New layout", "新布局"),
    ("New · Ctrl+N", "新建 · Ctrl+N"),
    ("Next page", "下一页"),
    ("No input state", "无输入时输出"),
    ("No plugin path is recorded.", "电路未记录此插件的路径。"),
    ("No tools available yet", "暂无可用元件"),
    ("No variables", "暂无变量"),
    ("None", "无"),
    ("Nop/Invalid Instr", "空操作/无效指令"),
    ("North", "北"),
    ("Number of stages", "级数"),
    ("Number of switches", "开关数量"),
    ("Numeric type", "数值类型"),
    ("OK", "确定"),
    ("OR Gate", "或门"),
    ("Octal", "八进制"),
    ("Odd Parity", "奇校验门"),
    ("Odd parity", "奇数个为 1"),
    ("Off color", "熄灭颜色"),
    ("Old behavior", "旧行为"),
    ("On color", "点亮颜色"),
    ("One", "补一"),
    (
        "Only stateless combinational circuits are supported. The current simulation is unchanged.",
        "仅支持无状态的组合电路。分析不改变当前仿真。",
    ),
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
    ("Output value", "输出值"),
    ("Output variables", "输出变量"),
    ("Output {}", "输出 {}"),
    ("P control (active low)", "P 控制端（低电平有效）"),
    ("P-Type", "P 型"),
    (
        "P: Poke · click a 1-bit input to toggle.",
        "P：交互 · 点击单比特输入切换电平。",
    ),
    ("PLA ROM", "PLA ROM"),
    ("PLA programming table", "PLA 编程表"),
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
    ("Pin {} · {}", "引脚 {} · {}"),
    ("Place instance", "放置实例"),
    ("Plexers", "复用器"),
    ("Plugin", "插件"),
    ("Plugin component", "插件元件"),
    (
        "Plugin identity or version does not match the circuit.",
        "插件标识或版本与电路不匹配。",
    ),
    ("Plugin ready", "插件已就绪"),
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
        "Poke: click to type. Enter sends a newline; Esc stops typing.",
        "交互：点击后输入文字，Enter 输入换行，Esc 结束输入。",
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
    (
        "Precedence: NOT > AND > XOR > OR. Use parentheses to group; constants are 0 and 1.",
        "优先级：非 > 与 > 异或 > 或。可使用括号，常量为 0 和 1。",
    ),
    ("Preset / clear positions", "置位 / 清零位置"),
    ("Preset: set all bits", "置位：异步置为全 1"),
    ("Previous page", "上一页"),
    ("Print…", "打印…"),
    ("Priority Encoder", "优先编码器"),
    ("Probe", "探针"),
    ("Product high", "乘积高位"),
    ("Product low", "乘积低位"),
    ("Product of sums", "或与"),
    ("Project", "项目"),
    ("Project Options…", "项目选项…"),
    ("Propagate one batch · Ctrl+I", "传播一个批次 · Ctrl+I"),
    ("Propagation limit — paused", "达到传播上限 — 已暂停"),
    (
        "Propagation limit: check feedback, then Reset or Run",
        "达到传播上限：请检查反馈，再重置或运行",
    ),
    ("Pull", "拉电阻端口"),
    ("Pull Resistor", "上拉 / 下拉电阻"),
    ("Pull behavior", "拉值行为"),
    ("Pull direction", "拉电阻方向"),
    ("Pull down", "下拉"),
    ("Pull up", "上拉"),
    ("Quad 2-input AND", "四路二输入与门"),
    ("Quad 2-input NAND", "四路二输入与非门"),
    ("Quad 2-input NOR", "四路二输入或非门"),
    ("Quad 2-input OR", "四路二输入或门"),
    ("Quad 2-input XNOR", "四路二输入同或门"),
    ("Quad 2-input XOR", "四路二输入异或门"),
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
    ("Random Generator", "随机数发生器"),
    ("Read address A", "读地址 A"),
    ("Read address B", "读地址 B"),
    ("Read enable", "读使能"),
    ("Rect", "矩形"),
    ("Rectangle", "矩形"),
    ("Red", "红色"),
    ("Redo · Ctrl+Shift+Z", "重做 · Ctrl+Shift+Z"),
    ("Refresh", "刷新"),
    ("Register", "寄存器"),
    ("Register contents", "寄存器内容"),
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
    ("Replace circuit: {}", "覆盖电路：{}"),
    ("Replace existing file?", "替换已有文件？"),
    (
        "Replace the artwork with a labeled package. You can undo this change.",
        "用带标签的封装替换外观，可以撤销。",
    ),
    (
        "Replaces all words; missing addresses become zero.",
        "覆盖全部内容，未提供的地址补零。",
    ),
    ("Reset", "重置"),
    ("Reset Simulation · Ctrl+R", "重置仿真 · Ctrl+R"),
    ("Reset canvas", "重置画布"),
    ("Reset canvas · Ctrl+0", "重置画布 · Ctrl+0"),
    (
        "Reset inputs, storage and clocks · Ctrl+R",
        "重置输入、存储状态和时钟 · Ctrl+R",
    ),
    (
        "Resizing preserves the upper-left connections; removed cells are discarded.",
        "调整尺寸保留左上区域的连接，移除的单元会被丢弃。",
    ),
    ("Result", "结果"),
    ("Retry", "重试"),
    ("Return to Parent · Alt+Left", "返回父电路 · Alt+Left"),
    (
        "Return to parent instance · Alt+Left",
        "返回父实例 · Alt+Left",
    ),
    ("Reverse: 1 counts down", "反向：1 时递减"),
    ("Revert", "还原"),
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
    (
        "Runtime values only; reset clears registers. Register 0 is read-only.",
        "仅修改运行值；复位清空寄存器。零号寄存器只读。",
    ),
    ("S-R Flip-Flop", "SR 触发器"),
    ("S-R Latch", "S-R 锁存器"),
    ("Save", "保存"),
    ("Save As… · Ctrl+Shift+S", "另存为… · Ctrl+Shift+S"),
    ("Save changes?", "保存修改？"),
    ("Save circuit as", "电路另存为"),
    ("Save · Ctrl+S", "保存 · Ctrl+S"),
    ("Search type", "查找类型"),
    ("Seed (0 = automatic)", "种子（0 为自动）"),
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
    ("Set input to X", "将输入设为 X"),
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
    ("Show internal structure", "显示内部结构"),
    ("Showing the first 32 issues.", "仅显示前 32 个问题。"),
    ("Sign", "符号位"),
    ("Signal: {}", "信号：{}"),
    ("Signed decimal", "有符号十进制"),
    (
        "Simplification exceeded its work or size limit. The truth table is preserved.",
        "化简超出计算量或规模限制，真值表已保留。",
    ),
    ("Simplified mode", "简化模式"),
    ("Simplify", "化简"),
    ("Simulate", "仿真"),
    ("Simulation issues ({})", "仿真问题（{}）"),
    ("Simulation mode", "仿真模式"),
    (
        "Simulation worker stopped. Restart the application.",
        "仿真工作线程已停止，请重新启动应用。",
    ),
    ("Size ", "字号 "),
    ("Slider", "滑块"),
    ("Snap", "网格吸附"),
    ("South", "南"),
    ("Splitter", "分线器"),
    ("Square", "方形"),
    ("Stable state", "稳定态"),
    (
        "Stable state settles combinational logic before sampling state. Unsupported circuits use event propagation.",
        "稳定态先收敛组合逻辑，再采样状态；不支持的电路使用事件传播。",
    ),
    ("Start", "起点"),
    ("Start: {}, {}", "起点：{}, {}"),
    ("Stay at value", "保持边界值"),
    ("Step", "单步"),
    ("Step Simulation · Ctrl+I", "单步仿真 · Ctrl+I"),
    ("Stick color", "摇杆颜色"),
    ("Store: 0 holds data", "存储：0 保持数据"),
    ("Stored value: {}", "存储值：{}"),
    ("Style", "样式"),
    ("Subcircuit", "子电路"),
    ("Subtractor", "减法器"),
    (
        "Successful clock ticks per second, measured over about one second. Includes manual ticks; not full clock cycles or propagation steps.",
        "按约一秒内成功推进的时钟步数统计，包含手动步进；不表示完整时钟周期数或信号传播步骤数。",
    ),
    ("Sum of products", "与或"),
    ("Switch", "开关"),
    (
        "Switching to expressions resolves all x entries to 0 or 1.",
        "切换到表达式时，所有 x 项会确定为 0 或 1。",
    ),
    ("T Flip-Flop", "T 触发器"),
    ("TTL", "TTL"),
    ("TTY", "TTY 字符终端"),
    ("Table row is out of range.", "真值表行号超出范围。"),
    ("Target: {} Hz", "设定：{} Hz"),
    ("Term", "项"),
    ("Term {} · input bit {}", "第 {} 项 · 输入位 {}"),
    ("Term {} · output bit {}", "第 {} 项 · 输出位 {}"),
    ("Text", "文字"),
    ("Text color", "文字颜色"),
    ("Text · T", "文字 · T"),
    (
        "The RAM simulation instance is no longer available.",
        "目标 RAM 仿真实例已不可用。",
    ),
    (
        "The analyzed interface no longer matches the circuit. Please analyze it again.",
        "分析接口已与电路不符，请重新分析。",
    ),
    (
        "The anchor sets the placement origin and the artwork's original facing.",
        "锚点决定放置原点与外观的原始朝向。",
    ),
    (
        "The circuit changed during analysis. Please analyze it again.",
        "分析期间电路发生变化，请重新分析。",
    ),
    (
        "The destination changed, cannot be read, or needs overwrite confirmation. Use Save As and choose the file again.",
        "目标文件已更改、无法读取或需要确认覆盖。请使用“另存为”重新选择。",
    ),
    (
        "The first input is the most significant table bit. Removing an input turns conflicting rows into x.",
        "第一个输入为真值表最高位。删除输入时，结果冲突的行将合并为 x。",
    ),
    (
        "The generated circuit would exceed the project item limit.",
        "生成电路后将超出项目项数限制。",
    ),
    (
        "The highlight follows the connected net, including matching tunnel labels.",
        "高亮显示整个相连网络，包括同名隧道。",
    ),
    (
        "The library would exceed the project circuit limit.",
        "导入此库将超出项目电路数量限制。",
    ),
    (
        "The memory dimensions are not supported.",
        "不支持此存储器的地址或数据位宽。",
    ),
    (
        "The memory image is not valid UTF-8 text.",
        "存储器映像不是有效的 UTF-8 文本。",
    ),
    (
        "The memory target changed. Open its contents editor and try again.",
        "目标存储器已改变，请重新打开其内容编辑器后重试。",
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
        "This input is used by an expression. Edit the expression before removing it.",
        "表达式仍使用此输入，请先修改表达式再删除。",
    ),
    (
        "This project has unsaved changes.",
        "此项目有未保存的修改。",
    ),
    (
        "This removes the circuit and its contents. You can undo this change.",
        "这会删除电路及其内容，可以撤销。",
    ),
    ("Three-state input", "三态输入"),
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
    ("Top/Left", "上/左"),
    ("Transistor", "晶体管"),
    ("Transistor type", "晶体管类型"),
    ("Transmission Gate", "传输门"),
    ("Trigger", "触发方式"),
    ("Truth Table", "真值表"),
    (
        "Truth table generation exceeded its work limit. The expressions are preserved.",
        "真值表生成超出计算量限制，表达式已保留。",
    ),
    (
        "Truth tables: 12 inputs · Expressions: 32 inputs · Outputs: 32",
        "真值表：12 输入 · 表达式：32 输入 · 输出：32",
    ),
    ("Tunnel", "隧道"),
    ("Tunnel · U", "隧道 · U"),
    ("Two's complement", "补码"),
    ("UTF-16 (16-bit)", "UTF-16（16 位）"),
    ("Unchanged", "保持不变"),
    ("Undo · Ctrl+Z", "撤销 · Ctrl+Z"),
    (
        "Unexpected expression token at character:",
        "表达式中出现意外符号，字符位置：",
    ),
    ("Unknown input variable:", "未知输入变量："),
    (
        "Unknown or invalid language preference.",
        "语言配置未知或无效。",
    ),
    ("Unknown or invalid settings.", "未知或无效的设置。"),
    ("Unsigned", "无符号"),
    ("Unsigned decimal", "无符号十进制"),
    (
        "Unsupported memory image format. Expected v2.0 raw on the first line.",
        "不支持此存储器映像格式，首行应为 v2.0 raw。",
    ),
    ("Untitled", "未命名"),
    ("Up", "上级目录"),
    ("Upper-left segment (f)", "左上段（f）"),
    ("Upper-right segment (b)", "右上段（b）"),
    (
        "Use File to open or save the supported .circ subset.",
        "使用文件菜单打开或保存支持的 .circ 文件。",
    ),
    ("Use NAND gates only", "仅使用 NAND 门"),
    (
        "Use a letter followed by letters, digits or underscores (up to 32 characters).",
        "名称须以字母开头，仅含字母、数字或下划线（最多 32 个字符）。",
    ),
    ("Use two-input gates only", "仅使用双输入门"),
    (
        "User configuration directory is unavailable.",
        "无法确定用户配置目录。",
    ),
    ("Value (hex)", "数值（十六进制）"),
    ("Variable index is out of range.", "变量索引超出范围。"),
    ("Variable name", "变量名称"),
    ("Vertex {}", "顶点 {}"),
    ("Vertical", "垂直对齐"),
    ("View instance", "查看实例"),
    ("View {}", "查看 {}"),
    (
        "Viewing this instance. Edits change the shared circuit definition.",
        "正在查看此实例。编辑会修改共享的电路定义。",
    ),
    (
        "Wasm plugins provide component graphics and behavior.",
        "Wasm 插件提供元件图形与仿真行为。",
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
    ("Without enable", "无使能"),
    ("Wrap around", "回绕"),
    ("Write data", "写数据"),
    ("Write enable", "写使能"),
    ("Write word", "写入单字"),
    ("XNOR Gate", "同或门"),
    ("XOR Gate", "异或门"),
    ("XOR mode", "异或模式"),
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
    ("floating/1", "浮空/1"),
    ("settled", "已稳定"),
    ("{} bit", "{} 位"),
    ("{} bit: {}", "{} 位：{}"),
    ("{} clock ticks · {} ticks/s", "时钟已步进 {} 次 · {} 步/秒"),
    ("{} components · {} wires", "{} 个元件 · {} 条导线"),
    (
        "{} inputs · {} AND terms · {} outputs",
        "{} 个输入 · {} 个 AND 项 · {} 个输出",
    ),
    (
        "{} interface pins · edit Pin components in the circuit definition",
        "{} 个接口引脚 · 可在电路定义中编辑引脚元件",
    ),
    ("{} pending", "{} 项待传播"),
    ("{} plugin error(s)", "{} 个插件错误"),
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
