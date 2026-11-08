//! Instance-local character buffers and clocked terminal screens.
use crate::{Bit, Simulator, Value};
use logisim_core::{Circuit, Component, Id, Kind, Trigger};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TtyScreen {
    pub lines: Vec<Vec<u16>>,
    pub row: usize,
    pub column: usize,
    columns: usize,
}
impl TtyScreen {
    fn new(c: &Component) -> Self {
        Self {
            lines: vec![vec![]; usize::from(c.terminal.rows.clamp(1, 48))],
            row: 0,
            column: 0,
            columns: usize::from(c.terminal.columns.clamp(1, 120)),
        }
    }
    fn clear(&mut self) {
        self.lines.iter_mut().for_each(Vec::clear);
        self.row = 0;
        self.column = 0;
    }
    fn resize(&mut self, c: &Component) {
        let rows = usize::from(c.terminal.rows.clamp(1, 48));
        if self.row >= rows {
            let skip = self.row + 1 - rows;
            self.lines.drain(..skip);
            self.row -= skip;
        }
        self.lines.resize(rows, vec![]);
        self.columns = usize::from(c.terminal.columns.clamp(1, 120));
        for line in &mut self.lines {
            line.truncate(self.columns);
        }
        self.column = self.column.min(self.columns);
    }
    fn newline(&mut self) {
        self.column = 0;
        self.row += 1;
        if self.row == self.lines.len() {
            self.lines.rotate_left(1);
            self.row -= 1;
        }
        self.lines[self.row].clear();
    }
    fn write(&mut self, value: u16) {
        match value {
            8 => {
                if self.column > 0 {
                    self.column -= 1;
                    self.lines[self.row].truncate(self.column);
                }
            }
            10 | 13 => self.newline(),
            12 => self.clear(),
            0..=31 | 127..=159 => {}
            _ => {
                if self.column == self.columns {
                    self.newline();
                }
                self.lines[self.row].push(value);
                self.column += 1;
            }
        }
    }
}
#[derive(Clone)]
pub(crate) struct State {
    kind: Kind,
    clock: Bit,
    buffer: VecDeque<u16>,
    screen: TtyScreen,
}
impl State {
    pub(crate) fn new(c: &Component) -> Self {
        Self {
            kind: c.kind,
            clock: Bit::Unknown,
            buffer: VecDeque::new(),
            screen: TtyScreen::new(c),
        }
    }
}
impl Simulator {
    pub fn keyboard_buffer(&self, id: Id) -> Option<Vec<u16>> {
        self.terminals
            .get(&id)
            .filter(|s| s.kind == Kind::Keyboard)
            .map(|s| s.buffer.iter().copied().collect())
    }
    pub fn tty(&self, id: Id) -> Option<&TtyScreen> {
        self.terminals
            .get(&id)
            .filter(|s| s.kind == Kind::Tty)
            .map(|s| &s.screen)
    }
    /// Queue a character without changing the document or manufacturing a clock edge.
    pub fn type_character(&mut self, id: Id, value: u16) {
        let Some(c) = self
            .circuit
            .component(id)
            .filter(|c| c.kind == Kind::Keyboard)
        else {
            return;
        };
        if self.port_value(id, 4).is_some_and(|v| v.bit(0) == Bit::One) {
            return;
        }
        let capacity = usize::from(c.terminal.buffer_len.clamp(1, 256));
        let Some(s) = self.terminals.get_mut(&id) else {
            return;
        };
        if s.buffer.len() == capacity {
            return;
        }
        s.buffer.push_back(value);
        self.limited = false;
        self.progress.restart();
        self.schedule_terminals(None);
    }
    pub(crate) fn synchronize_terminals(&mut self, circuit: &Circuit) {
        self.terminals = circuit
            .components
            .iter()
            .filter(|c| c.kind.is_terminal())
            .map(|c| {
                let mut s = self
                    .terminals
                    .remove(&c.id)
                    .filter(|s| s.kind == c.kind)
                    .unwrap_or_else(|| State::new(c));
                s.buffer
                    .truncate(usize::from(c.terminal.buffer_len.clamp(1, 256)));
                s.screen.resize(c);
                (c.id, s)
            })
            .collect();
    }
    pub(crate) fn schedule_terminals(&mut self, changed: Option<&crate::work::WorkSet>) {
        // Sample all input nets before changing any runtime state or queuing outputs.
        let samples: Vec<_> = self
            .plan
            .terminals
            .iter()
            .filter(|entry| entry.affected(changed))
            .map(|entry| {
                let c = &self.circuit.components[entry.component];
                let clock_index = if c.kind == Kind::Keyboard { 2 } else { 1 };
                let read = |i| entry.port(i, &self.values);
                (
                    c.id,
                    c.bits,
                    c.terminal.trigger,
                    read(0),
                    read(clock_index).bit(0),
                    read(clock_index + 1).bit(0),
                    read(clock_index + 2).bit(0),
                )
            })
            .collect();
        let mut outputs = vec![];
        for (id, bits, trigger, data, clock, enable, clear) in samples {
            let s = self.terminals.get_mut(&id).expect("terminal state");
            let edge = matches!(
                (trigger, s.clock, clock),
                (Trigger::Rising, Bit::Zero, Bit::One) | (Trigger::Falling, Bit::One, Bit::Zero)
            );
            s.clock = clock;
            if clear == Bit::One {
                s.buffer.clear();
                s.screen.clear();
            } else if edge && enable != Bit::Zero {
                if s.kind == Kind::Keyboard {
                    s.buffer.pop_front();
                } else {
                    s.screen
                        .write(data.number().map_or(u16::from(b'?'), |v| v as u16));
                }
            }
            if s.kind == Kind::Keyboard {
                outputs.push((
                    (id, 0),
                    Value::known(bits, u32::from(s.buffer.front().copied().unwrap_or(0))),
                ));
                outputs.push(((id, 1), Value::known(1, u32::from(!s.buffer.is_empty()))));
            }
        }
        for (port, value) in outputs {
            self.drive_port(port, value);
        }
    }
}
