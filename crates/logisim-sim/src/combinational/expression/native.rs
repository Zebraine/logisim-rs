//! Partition only direct-read pure instructions. No network, clock or hierarchy
//! pointers enter the generated code; all drives are materialized in the evaluator.
use super::*;
use crate::jit_experiment::{self, Mode};
use logisim_core::{Arithmetic, Gate, plugin::api::Signal};
use logisim_jit_spike::{Code, MAX_CHAIN, Operand, Region, Word};
use std::time::Instant;

struct Block {
    code: Arc<Code>,
    layout: BlockLayout,
}
pub(super) struct Native {
    starts: Vec<Option<Block>>,
    frame: Vec<Word>,
    profile: bool,
}

fn planes(value: Value) -> Word {
    let s = Signal::from(value);
    [s.value, s.unknown, s.error]
}

fn unary(inst: &Instruction) -> Option<(usize, usize, u8, bool)> {
    let Operation::Gate(g, present) = &inst.operation else {
        return None;
    };
    let [(1, Read::Direct(input))] = inst.inputs.as_slice() else {
        return None;
    };
    let [Some(output), None] = inst.outputs.as_ref() else {
        return None;
    };
    (g.gate.unary()
        && !g.input_negated(0)
        && present.as_ref() == [true]
        && (1..=32).contains(&g.bits))
    .then_some((*input, *output, g.bits, g.gate == Gate::Not))
}

fn unary_block(insts: &[Instruction]) -> Option<(Region, BlockLayout)> {
    let (first, _, width, _) = unary(&insts[0])?;
    let mut previous = first;
    let mut invert = Vec::new();
    let mut outputs = Vec::new();
    for inst in insts.iter().take(MAX_CHAIN) {
        let Some((input, output, bits, not)) = unary(inst) else {
            break;
        };
        if input != previous || bits != width {
            break;
        }
        previous = output;
        invert.push(not);
        outputs.push((output, width));
    }
    let nodes = invert.len();
    Some((
        Region::Unary { width, invert },
        BlockLayout {
            inputs: vec![(first, 0)],
            output_start: 1,
            outputs,
            nodes,
        },
    ))
}

struct BlockLayout {
    inputs: Vec<(usize, usize)>,
    output_start: usize,
    outputs: Vec<(usize, u8)>,
    nodes: usize,
}

fn add_block(inst: &Instruction) -> Option<(Region, BlockLayout)> {
    let Operation::Arithmetic(g) = &inst.operation else {
        return None;
    };
    if g.kind != Arithmetic::Adder || !(1..=32).contains(&g.bits) {
        return None;
    }
    let [Some(sum), None, None, None, Some(carry)] = inst.outputs.as_ref() else {
        return None;
    };
    let [(1, a), (2, b), (3, c)] = inst.inputs.as_slice() else {
        return None;
    };
    let mut operands = [Operand::Input, Operand::Input, Operand::Input];
    let mut inputs = Vec::new();
    for (index, read) in [a, b, c].into_iter().enumerate() {
        match read {
            Read::Direct(slot) => inputs.push((*slot, index)),
            Read::Constant(v) => operands[index] = Operand::Constant(planes(*v)),
            Read::Gather(_) => return None,
        }
    }
    Some((
        Region::Add {
            width: g.bits,
            operands,
        },
        BlockLayout {
            inputs,
            output_start: 3,
            outputs: vec![(*sum, g.bits), (*carry, 1)],
            nodes: 1,
        },
    ))
}

impl Native {
    pub(super) fn new(program: &Program, mode: Mode) -> Option<Self> {
        let mut starts: Vec<_> = (0..program.instructions.len()).map(|_| None).collect();
        let mut words = 0;
        let mut i = 0;
        let mut nodes = 0;
        let mut regions = 0;
        while i < starts.len() {
            let rest = &program.instructions[i..];
            let Some((spec, layout)) = unary_block(rest).or_else(|| add_block(&rest[0])) else {
                i += 1;
                continue;
            };
            let count = layout.nodes;
            if let Some(code) = jit_experiment::kernel(spec) {
                words = words.max(layout.output_start + layout.outputs.len());
                starts[i] = Some(Block { code, layout });
                nodes += count;
                regions += 1;
            }
            i += count;
        }
        jit_experiment::record(|s| {
            s.native_nodes += nodes;
            s.native_regions += regions
        });
        (regions > 0).then(|| Self {
            starts,
            frame: vec![[0; 3]; words],
            profile: mode.profile(),
        })
    }

    /// Each internal node of a fused chain has exactly one predecessor (the
    /// previous output). No other boundary can independently enqueue its interior.
    pub(super) fn execute(
        &mut self,
        index: usize,
        values: &mut [Value],
        mut changed: impl FnMut(usize, usize, Value, Value),
    ) -> Option<usize> {
        let Block {
            code,
            layout: block,
        } = self.starts[index].as_ref()?;
        let start = self.profile.then(Instant::now);
        let frame = &mut self.frame[..block.output_start + block.outputs.len()];
        for &(slot, word) in &block.inputs {
            frame[word] = planes(values[slot])
        }
        code.run(frame);
        for (&(slot, width), &[value, unknown, error]) in
            block.outputs.iter().zip(&frame[block.output_start..])
        {
            let new = Value::from(Signal {
                width,
                value,
                unknown,
                error,
            });
            let old = values[slot];
            if old != new {
                values[slot] = new;
                changed(index + block.nodes, slot, old, new)
            }
        }
        if let Some(start) = start {
            let ns = start.elapsed().as_nanos() as u64;
            jit_experiment::record(|s| {
                s.native_region_ns += ns;
                s.native_instructions += block.nodes;
                s.native_calls += 1
            });
        }
        Some(block.nodes)
    }
}
