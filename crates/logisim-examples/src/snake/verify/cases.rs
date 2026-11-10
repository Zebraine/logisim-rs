use super::*;
pub(super) fn play(
    project: &Project,
    plugins: &logisim_plugin::Registry,
    stable: bool,
) -> Result<()> {
    let mut g = Game::new(project, plugins, stable, vec![])?;
    assert_eq!(g.indicator("SCORE"), 0);
    g.ready()?;
    assert_eq!(g.indicator("SCORE"), 0);
    let initial = VecDeque::from([0x85, 0x86, 0x87]);
    let mut body = initial.clone();
    g.assert_body(&body);
    g.press(Key::ArrowLeft)?;
    let food = g.reg("food");
    g.moved()?;
    step(&mut body, 0, food);
    g.assert_body(&body);
    g.press(Key::ArrowUp)?;
    g.press(Key::ArrowLeft)?;
    for dir in [3, 2] {
        let food = g.reg("food");
        g.moved()?;
        step(&mut body, dir, food);
        assert_eq!(g.reg("dir"), dir);
        g.assert_body(&body);
    }
    g.press(Key::Space)?;
    g.until(80, |g| g.reg("paused") == 1)?;
    assert_eq!(g.indicator("PAUSED"), 1);
    let frame = g.image();
    let head = g.reg("head");
    g.press(Key::ArrowDown)?;
    g.press(Key::ArrowRight)?;
    for _ in 0..150 {
        g.cycle()?;
    }
    assert_eq!(g.reg("head"), head);
    assert!(Arc::ptr_eq(&frame, &g.image()));
    g.press(Key::Space)?;
    g.until(80, |g| g.reg("paused") == 0)?;
    let food = g.reg("food");
    g.moved()?;
    step(&mut body, 2, food);
    g.assert_body(&body);
    g.press(Key::Enter)?;
    g.until(80, |g| g.reg("phase") != 8)?;
    g.ready()?;
    g.assert_body(&initial);
    body = initial.clone();
    for _ in 0..8 {
        let food = g.reg("food");
        g.moved()?;
        step(&mut body, 0, food);
        g.assert_body(&body);
    }
    g.moved()?;
    assert_eq!(g.reg("phase"), 16);
    assert_eq!(g.indicator("DEAD"), 1);
    let head = g.reg("head");
    let frame = g.image();
    for _ in 0..20 {
        g.cycle()?;
    }
    assert_eq!(g.reg("head"), head);
    assert!(Arc::ptr_eq(&frame, &g.image()));
    g.press(Key::Enter)?;
    g.until(40, |g| g.reg("phase") != 16)?;
    g.ready()?;
    g.assert_body(&initial);
    g.input(InputEvent::Key {
        key: Key::ArrowUp,
        pressed: true,
    })?;
    g.input(InputEvent::ReleaseAll {})?;
    let food = g.reg("food");
    g.moved()?;
    let mut b = initial;
    step(&mut b, 0, food);
    g.assert_body(&b);
    Ok(())
}
pub(super) fn boundaries(
    input: &Plugin,
    display: &Plugin,
    plugins: &logisim_plugin::Registry,
    stable: bool,
) -> Result<()> {
    // Growth by driving a one-cell snake along a simple path to the real random food.
    let project = generate(
        input,
        display,
        &Start {
            body: vec![0],
            direction: 0,
            delay: 3,
        },
    )?;
    let mut g = Game::new(&project, plugins, stable, vec![])?;
    g.ready()?;
    let target = g.reg("food");
    let mut body = VecDeque::from([0]);
    for (dir, key, count) in [
        (0, Key::ArrowRight, target & 15),
        (1, Key::ArrowDown, target >> 4),
    ] {
        if count == 0 {
            continue;
        }
        g.press(key)?;
        for _ in 0..count {
            let food = g.reg("food");
            g.moved()?;
            step(&mut body, dir, food);
            g.assert_body(&body);
        }
    }
    assert_eq!(g.reg("length"), 2);
    assert_eq!(g.indicator("SCORE"), 1);
    // A square permits moving into the cell being vacated by the tail.
    let start = Start {
        body: vec![0x11, 0x21, 0x22, 0x12],
        direction: 3,
        delay: 63,
    };
    let project = generate(input, display, &start)?;
    let mut g = Game::new(&project, plugins, stable, vec![])?;
    g.ready()?;
    g.press(Key::ArrowLeft)?;
    let food = g.reg("food");
    let mut body = VecDeque::from(start.body);
    g.moved()?;
    step(&mut body, 2, food);
    g.assert_body(&body);
    assert_eq!(g.reg("head"), 0x11);
    let project = generate(
        input,
        display,
        &Start {
            body: vec![0x11, 0x12, 0x13, 0x23, 0x22],
            direction: 2,
            delay: 63,
        },
    )?;
    let mut g = Game::new(&project, plugins, stable, vec![])?;
    g.ready()?;
    g.press(Key::ArrowUp)?;
    g.moved()?;
    assert_eq!(g.reg("phase"), 16);
    assert_eq!(g.reg("head"), 0x22);
    // Start one cell short of a full board; the only free cell must become food.
    let path: Vec<u8> = (0..16)
        .flat_map(|y| (0..16).map(move |x| (y * 16 + if y % 2 == 0 { x } else { 15 - x }) as u8))
        .collect();
    let project = generate(
        input,
        display,
        &Start {
            body: path[..255].to_vec(),
            direction: 2,
            delay: 3,
        },
    )?;
    let mut g = Game::new(&project, plugins, stable, vec![])?;
    g.ready()?;
    assert_eq!(g.reg("food"), 0xf0);
    g.moved()?;
    assert_eq!(g.reg("phase"), 15);
    assert_eq!(g.indicator("WON"), 1);
    assert_eq!(g.indicator("SCORE"), 1);
    g.assert_body(&VecDeque::from(path));
    // Corrupted/manual RAM contents cannot make food search loop forever.
    let project = generate(input, display, &Start::default())?;
    let mut g = Game::new(&project, plugins, stable, vec![])?;
    g.until(20, |g| g.reg("phase") == 4)?;
    g.sim
        .fill_memory(&g.path, g.memories["occupied"], 0, 256, 1);
    g.settle()?;
    g.until(260, |g| g.reg("phase") == 15)?;
    // Two instances of the same entire game share definitions, never state.
    let mut project = project;
    let game = project.main;
    let root = project.add("Two games".into())?;
    project.main = root;
    let a = project.instance(game, Point::new(100, 100)).unwrap();
    let b = project.instance(game, Point::new(500, 100)).unwrap();
    let a = project.circuit_mut(root).unwrap().circuit.add_component(a);
    let b = project.circuit_mut(root).unwrap().circuit.add_component(b);
    project.refresh_interfaces(false);
    let mut g = Game::new(&project, plugins, stable, vec![a])?;
    g.ready()?;
    g.press(Key::ArrowUp)?;
    g.moved()?;
    let other = [b, *g.path.last().unwrap()];
    assert_eq!(
        g.sim
            .view(&other)
            .unwrap()
            .stored_value(g.registers["dir"])
            .unwrap()
            .number(),
        Some(0)
    );
    assert_eq!(g.reg("dir"), 3);
    Ok(())
}
