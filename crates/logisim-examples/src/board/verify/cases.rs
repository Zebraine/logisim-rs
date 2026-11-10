use super::*;
use logisim_core::plugin::api::interaction::Key;
pub(super) fn trace(
    input: &Plugin,
    display: &Plugin,
    plugins: &logisim_plugin::Registry,
    stable: bool,
) -> Result<()> {
    let mut source = String::from("LI r0,123\nLI r1,65535\nLI r2,1\n");
    for op in [
        "ADD", "SUB", "AND", "OR", "XOR", "SHL", "SHR", "SAR", "SLTU", "SLT",
    ] {
        source.push_str(&format!("{op} r3,r1,r2\nST r3,r0,32\n"));
    }
    source.push_str("ADDI r1,r1,1\nADDI r1,r1,-1\nANDI r2,r1,0xf0\nORI r2,r2,0x0f\nXORI r2,r2,0xffff\nLI r5,0xfffe\nST r2,r5,3\nLD r4,r0,1\nST r4,r0,4095\nLD r4,r0,4095\nST r4,r0,4096\nLD r4,r0,4096\nST r2,r0,0xff30\nLD r6,r0,0xff30\nBZ r4,taken\nHALT\ntaken: BNZ r4,wrong\nBNZ r1,call\nwrong: HALT\ncall: JAL r7,sub\nJMP after\nsub: ADDI r6,r6,1\nJR r7\nafter: NOP\n");
    let mut seed = 0x12345678u32;
    for _ in 0..64 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let op = [
            "ADD", "SUB", "AND", "OR", "XOR", "SHL", "SHR", "SAR", "SLTU", "SLT",
        ][(seed as usize) % 10];
        source.push_str(&format!(
            "{op} r{},r{},r{}\n",
            (seed >> 4) & 7,
            (seed >> 8) & 7,
            (seed >> 12) & 7
        ));
    }
    source.push_str("HALT\n");
    for (program, steps) in [
        (source.as_str(), 180),
        ("JMP edge\n.org 65535\nedge: NOP", 5),
        (".word 0xb8000000", 6),
        ("LI r1,3\n.word 0x89040000", 6),
        (".word 0x10000001", 6),
    ] {
        let words = assembler::assemble(program)?;
        let project = generate(input, display, &words)?;
        let mut m = Machine::new(&project, plugins, stable, vec![])?;
        let mut model = Model::default();
        compare(&m, &model);
        for _ in 0..steps {
            let word = words.get(model.pc as usize).copied().unwrap_or(0);
            model.step(word);
            m.instruction()?;
            compare(&m, &model);
        }
        m.reset()?;
        compare(&m, &Model::default());
    }
    Ok(())
}
pub(super) fn peripherals(
    project: &Project,
    plugins: &logisim_plugin::Registry,
    stable: bool,
) -> Result<()> {
    let mut m = Machine::new(project, plugins, stable, vec![])?;
    m.until(200, |m| m.io("debug") == 0x38)?;
    assert_eq!(m.word(16), 0xbeef);
    assert_eq!(m.pixel(8, 8), [0, 255, 0, 255]);
    assert_eq!(m.pixel(0, 0), [0, 0, 0, 255]);
    for (key, x, y) in [
        (Key::ArrowUp, 8, 7),
        (Key::ArrowLeft, 7, 7),
        (Key::ArrowDown, 7, 8),
        (Key::ArrowRight, 8, 8),
    ] {
        m.press(key)?;
        m.until(250, |m| {
            m.io("x") == x && m.io("y") == y && m.pixel(x as usize, y as usize) == [0, 255, 0, 255]
        })?;
    }
    for _ in 0..100 {
        m.cycle()?;
    }
    assert_eq!(m.io("x"), 8);
    assert_eq!(m.io("y"), 8);
    assert_eq!(m.state("fault"), 0);
    m.reset()?;
    assert_eq!(m.word(16), 0);
    assert_eq!(m.state("pc"), 0);
    assert_eq!(m.io("debug"), 0);
    m.until(200, |m| m.io("debug") == 0x38)?;
    // Two identical boards must have separate CPU, RAM and peripheral state.
    let mut project = project.clone();
    let main = project.main;
    let top = project.add("Two boards".into())?;
    project.main = top;
    let a = project.instance(main, Point::new(0, 0)).unwrap();
    let b = project.instance(main, Point::new(800, 0)).unwrap();
    let a = project.circuit_mut(top).unwrap().circuit.add_component(a);
    let b = project.circuit_mut(top).unwrap().circuit.add_component(b);
    project.refresh_interfaces(false);
    let mut m = Machine::new(&project, plugins, stable, vec![a])?;
    m.until(200, |m| m.io("debug") == 0x38)?;
    m.press(Key::ArrowUp)?;
    m.until(250, |m| m.io("y") == 7)?;
    let mut other = m.io.clone();
    other[0] = b;
    let def = &project.named("G16 memory and IO").unwrap().circuit;
    let y = def
        .components
        .iter()
        .find(|c| c.kind == Kind::Register && c.label == "y")
        .unwrap()
        .id;
    assert_eq!(
        m.sim
            .view(&other)
            .unwrap()
            .stored_value(y)
            .unwrap()
            .number(),
        Some(8)
    );
    Ok(())
}
pub(super) fn replacement(
    input: &Plugin,
    display: &Plugin,
    plugins: &logisim_plugin::Registry,
    stable: bool,
) -> Result<()> {
    let words = assembler::assemble("LI r1,224\nST r1,r0,0xff12\nLI r1,6\nST r1,r0,0xff13\nHALT")?;
    let project = generate(input, display, &words)?;
    let mut m = Machine::new(&project, plugins, stable, vec![])?;
    m.until(40, |m| m.state("halted") == 1)?;
    for y in 0..16 {
        for x in 0..16 {
            assert_eq!(m.pixel(x, y), [255, 0, 0, 255]);
        }
    }
    let before = (m.state("pc"), m.register(1));
    for _ in 0..10 {
        m.cycle()?;
    }
    assert_eq!((m.state("pc"), m.register(1)), before);
    Ok(())
}
