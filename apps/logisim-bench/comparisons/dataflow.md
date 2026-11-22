# Stage 16 · Single-circuit stable dataflow

**No-Go.** Four lanes reduced course CPU throughput by 11.2% on wasmi and 29.8% on cached Wasmtime relative to the serial control. The fixed wide and deep traces exposed one ready node at a time. Stop this experiment here and retain the serial evaluator in product builds.

## Measured parallelism

A region is a weakly connected group of eligible compiled instructions within one circuit instance. W counts instructions actually executed. L sums the critical-path lengths of their active dependency graphs, separately for every pass and instance. Each eligible instruction costs one in this calculation. Consecutive settles and calls across a clock, instance or plugin boundary cannot overlap. W/L is an optimistic width, before unequal instruction costs, scheduling, publication or serial simulation work.

| Hot trace | W | L | W/L | Peak nodes / regions in an active layer | Dispatched nodes / barriers |
| --- | ---: | ---: | ---: | ---: | ---: |
| Wide, 512 input changes | 512 | 512 | 1.00 | 1 / 1 | 0 / 0 |
| Deep, 512 input changes | 162,816 | 162,816 | 1.00 | 1 / 1 | 0 / 0 |
| Course CPU, 128 ticks | 7,279 | 2,182 | 3.34 | 19 / 9 | 5,642 / 1,148 |
| Register file, 512 ticks | 0 | 0 | — | 0 / 0 | 0 / 0 |

Wide has 64 independent static lanes, and changes one input per iteration. Deep is a chain of 318 inversions. The register file executed 256 plugin outputs and 1,024 samples with zero compiled expression work. The CPU issued 4,626 local evaluator calls, of which 751 performed eligible work. Its 2,182 active layers averaged **1.42 distinct regions**, with median 1 and p95 3. The distribution of region counts 1/2/3/4/5/6/7/9 was 1,706/240/135/54/29/4/1/13. The same layer can contain independent instructions in a single region.

On the CPU, 6,375 eligible executions (87.6%) fell in dynamic layers of width two or greater. The compiled static-layer schedule dispatched 5,642 (77.5%). Mixed ROM/PLA layers and sparse work can fall back to serial execution. A further 64 excluded instruction executions retained their dependencies. An ideal barrier per *active* level with identical node costs needs 7,279 / 4,408 / 3,100 instruction time slots at 1 / 2 / 4 lanes respectively. This upper-bound calculation omits the plugin's 159 outputs and 191 samples, state commits and host-side net handling.

## Linux release measurements

Intel Core i5-12600KF, Linux x86-64, Rust 1.99.0, affinity CPUs 12–15 (four E-cores). Baseline used the Stage 15 evaluator at `3a9fd28` with both dataflow features off. The candidate used `ed6b759` with `dataflow-parallel`. One lane uses the serial path in that binary. The fixed CPU set applied to the main thread and all helpers. Each cell is the median of three fresh processes with three warmup traces and 20 measured traces per process. The variant order rotated by trial. Both binaries used the same existing plugin metrics required by the cache study. Wasmtime timed opens for CPU and register file all hit a primed private cache. Wide and deep do not load a Wasm module.

Each cell gives **iterations/s · settle p50/p95 in µs**. An iteration is one tick on the clocked fixtures and one input change on the combinational fixtures. Register file settles twice per tick. Rates include driving inputs and the complete settle or tick sequence. Probe hashing, startup, trace reset and the observation pass are outside hot iteration timing.

| Workload | Engine request | Serial control | 1 lane | 2 lanes | 4 lanes |
| --- | --- | ---: | ---: | ---: | ---: |
| Wide | wasmi | 258,259 · 3.48/3.76 | 256,826 · 3.49/3.79 | 255,794 · 3.47/3.80 | 254,481 · 3.52/3.84 |
| Wide | wasmtime | 257,816 · 3.48/3.80 | 257,641 · 3.48/3.77 | 253,551 · 3.50/3.85 | 262,315 · 3.45/3.74 |
| Deep | wasmi | 12,682 · 76.80/83.86 | 11,969 · 84.23/87.75 | 11,684 · 86.28/89.02 | 11,403 · 89.13/92.32 |
| Deep | wasmtime | 12,513 · 80.66/83.53 | 11,788 · 85.55/88.53 | 11,341 · 86.97/93.07 | 11,631 · 85.48/89.55 |
| Course CPU | wasmi | 2,358 · 136.22/881.34 | 2,358 · 144.23/881.54 | 2,099 · 140.83/1009.48 | 2,094 · 153.10/1012.66 |
| Course CPU | wasmtime | 8,851 · 31.90/244.90 | 8,721 · 32.32/249.25 | 6,274 · 35.90/383.18 | 6,212 · 49.04/388.72 |
| Register file | wasmi | 3,059 · 113.20/324.33 | 3,028 · 114.03/324.01 | 3,077 · 113.20/320.39 | 3,077 · 113.37/322.61 |
| Register file | wasmtime | 37,200 · 8.93/27.10 | 35,384 · 9.33/28.00 | 35,975 · 9.28/27.50 | 36,386 · 9.19/27.55 |

Whole-process CPU utilization includes startup, checks and reset. 100% means one logical CPU.

| Workload | Engine request | Control | 1 lane | 2 lanes | 4 lanes |
| --- | --- | ---: | ---: | ---: | ---: |
| Wide | wasmi | 99.4% | 98.7% | 99.2% | 101.1% |
| Wide | wasmtime | 99.4% | 99.4% | 99.9% | 101.1% |
| Deep | wasmi | 99.3% | 99.8% | 99.8% | 99.8% |
| Deep | wasmtime | 99.7% | 99.7% | 99.7% | 99.8% |
| Course CPU | wasmi | 99.7% | 99.1% | 110.4% | 138.7% |
| Course CPU | wasmtime | 99.5% | 99.5% | 129.2% | 203.1% |
| Register file | wasmi | 99.8% | 99.8% | 99.8% | 99.8% |
| Register file | wasmtime | 99.7% | 99.1% | 99.3% | 99.2% |

The independent observational replay timed dispatch through barrier completion and each lane's gather, compute and staging. The difference estimates time outside the longest lane, including scheduling and waiting. Ordered merge after the barrier is additional work covered by the end-to-end timing. Times below are medians across three processes for one complete 128-tick CPU trace. The study timers were compiled out of the throughput binaries.

| ABI 4 engine | Lanes | Dispatch through barrier (ms) | Longest lane summed over barriers (ms) | Difference (µs/barrier) |
| --- | ---: | ---: | ---: | ---: |
| wasmi | 2 | 6.261 | 1.407 | 4.23 |
| wasmi | 4 | 6.724 | 1.184 | 4.83 |
| Wasmtime | 2 | 6.372 | 1.471 | 4.27 |
| Wasmtime | 4 | 6.519 | 1.081 | 4.74 |

The four-lane wasmi CPU ranged from 2,065–2,096 ticks/s across processes, below the control's 2,268–2,368. Wasmtime gave 6,091–6,237 versus 8,399–8,895. Even the best parallel runs missed the 1.10× CPU gate. Wide and deep missed 1.30×. The four-lane register-file medians stayed within 5% of their controls. Deep showed variation even across engine requests that load no plugin, so small differences between those rows do not establish engine effects. In a separate register-file replay, all three helpers were sleeping at both ends of a 0.5-second sample. Linux per-thread scheduler runtime advanced by zero for each.

## Execution and correctness

One simulation thread owns the evaluator. Up to three persistent helpers read the same immutable pre-layer slots, reuse the serial four-state operation kernels and stage results privately. The owner publishes completed writes in compiled layer, instruction and port order. A cancelled or panicking job drains the scoped work before serial replay of the unpublished layer. Dropping the pool joins the workers. Clock sampling, sequential commits, plugin calls, RAM and manual event steps stay on their original paths. ROM and PLA operations keep their layer serial. The experimental pool dependency is optional.

| Trace | Four-state digest | Ticks | Steps | Diagnostics |
| --- | --- | ---: | ---: | ---: |
| Wide | `a817e833dd81da75` | 0 | 512 | 0 |
| Deep | `17090361c775bb25` | 0 | 512 | 0 |
| Course CPU | `73171884c0465aad` | 128 | 771 | 0 |
| Register file | `a2689cffb70ff825` | 512 | 1,536 | 0 |

All timed and observational replays compared digest, ticks, steps, diagnostics and stable work counts. Regression tests additionally compare each net and driver in every CPU and register-file instance at each traced settle against serial execution, including the pre-clock data-bus settle. They exercise four-state reconvergence, cancellation and worker panic, feedback rejection, manual event steps and mixed memory frontiers.

Workspace verification passed formatting, all-target/all-feature check and clippy with warnings denied, the locked offline test suite (961 passed, one existing manual snapshot benchmark ignored), and the default release benchmark build. The default normal dependency trees for the simulator, desktop, Agent and benchmark exclude `rayon-core` and `crossbeam-deque`.

## Reproducing the comparison

Build the control in a checkout at `3a9fd28`. Run the next two builds in the experiment checkout. Preserve a copy of each executable before the next Cargo build replaces it.

```sh
cargo build --release --locked --offline -p logisim-bench --features wasmtime-cache
cp target/release/logisim-bench /tmp/dataflow-control

cargo build --release --locked --offline -p logisim-bench --features wasmtime-cache,dataflow-parallel
cp target/release/logisim-bench /tmp/dataflow-timed
cargo build --release --locked --offline -p logisim-bench --features wasmtime-cache,dataflow-parallel,dataflow-study
cp target/release/logisim-bench /tmp/dataflow-traced

python3 apps/logisim-bench/comparisons/dataflow.py --cpus 12-15 \
  --baseline /tmp/dataflow-control --timed /tmp/dataflow-timed --traced /tmp/dataflow-traced > /tmp/dataflow.jsonl
```

The script writes 96 timing records and 72 study records, including per-instance widths and per-process utilization. Select four available physical cores on another host. A private temporary module cache is primed once, checked in every applicable run and removed at exit.
