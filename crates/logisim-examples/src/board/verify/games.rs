//! Exercise the published software on the real CPU circuit and real Wasm peripherals.
mod pong;
mod snake;
use super::*;
use logisim_core::plugin::api::interaction::Key;

pub(super) fn check(
    boards: &[(Project, assembler::Assembly)],
    plugins: &logisim_plugin::Registry,
    stable: bool,
) -> Result<()> {
    snake::check(Game::new(&boards[0], plugins, stable)?)?;
    pong::check(Game::new(&boards[1], plugins, stable)?)?;
    println!(
        "{}: software Snake and Ping Pong gameplay passed",
        if stable { "Stable" } else { "Events" }
    );
    Ok(())
}
struct Game<'a> {
    m: Machine,
    code: &'a assembler::Assembly,
}
impl<'a> Game<'a> {
    fn new(
        board: &'a (Project, assembler::Assembly),
        plugins: &logisim_plugin::Registry,
        stable: bool,
    ) -> Result<Self> {
        let mut game = Self {
            m: Machine::new(&board.0, plugins, stable, vec![])?,
            code: &board.1,
        };
        game.at_loop()?;
        assert_eq!(game.get("READY"), 1);
        Ok(game)
    }
    fn address(&self, name: &str) -> u32 {
        self.code.symbols[name].try_into().unwrap()
    }
    fn get(&self, name: &str) -> u16 {
        self.m.word(self.address(name))
    }
    fn set(&mut self, name: &str, value: u16) -> Result<()> {
        self.m.set_word(self.address(name), value)
    }
    fn at_loop(&mut self) -> Result<()> {
        let pc = self.address("main_loop");
        self.m
            .until(10_000, |m| m.state("pc") == pc && m.state("phase") == 0)?;
        assert_eq!(self.m.state("fault"), 0);
        assert_eq!(self.m.state("halted"), 0);
        Ok(())
    }
    fn loop_once(&mut self) -> Result<()> {
        self.m.instruction()?;
        self.at_loop()
    }
    fn press(&mut self, key: Key) -> Result<()> {
        self.m.press(key)?;
        // Process both key-down and key-up through the CPU's explicit ACK stores.
        self.loop_once()?;
        self.loop_once()
    }
    fn frame(&mut self) -> Result<()> {
        let frame = self.get("FRAME");
        let counter = self.address("FRAME");
        self.m.until(20_000, |m| m.word(counter) != frame)?;
        self.at_loop()
    }
    fn due_frame(&mut self) -> Result<()> {
        let last = (self.m.io("timer") as u16).wrapping_sub(self.address("SPEED") as u16 + 1);
        self.set("LAST", last)?;
        self.frame()
    }
}
const BLACK: [u8; 4] = [0, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
