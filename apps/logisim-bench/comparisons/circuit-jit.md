# Stable region JIT experiment

**No-Go for production.** Native execution made the deep-chain evaluator 2.88× as fast, but the course CPU delivered 2,315 tick/s against 2,318 interpreted. The three paired CPU changes were −3.3%, −0.1% and +1.6%. This misses the 10% course target and falls inside the 5% stop band. End work on circuit JIT and whole-circuit AOT for this workload.

## Reproducing the comparison

The base was `f9e0566` and the measured implementation was the Linux x86-64 code in `816871f`. The i5-12600KF ran Linux 7.3.4 with the powersave governor, pinned to E-core 15. Builds used Rust 1.99.0, a single simulation thread and wasmi. Plugin metrics were disabled. Execute from the repository root.

```sh
cargo build --release -p logisim-bench --locked --offline --no-default-features --features circuit-jit-experiment
python3 apps/logisim-bench/comparisons/circuit-jit.py --core 15 --output experiments/circuit-jit
target/release/logisim-bench --circuit-jit verify single-cycle --runs 20
```

Each workload ran in three pairs of fresh processes with three warmup traces and 20 measured traces per process. The interpreted and native paths used the same release binary and alternated launch order. One trace contains 128 CPU ticks, 512 register-file ticks or 512 input changes on wide/deep. The tables give medians of process statistics. Each p50/p95 is first calculated within its process. OS file pages could be warm at process start. The runner saves individual JSON results and compares the trace fingerprints.

## End-to-end results

Rates include the input change or tick and all required settles. Each timing pair below is interpreted → native. The median rate of the pre-change, feature-disabled binary on the same machine was 272,913 / 12,270 / 2,315 / 3,065 in table order.

| Workload | Iterations/s | Rate change | Settle p50 (µs) | Settle p95 (µs) | Steps / ticks per trace | Four-state digest |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Wide | 270,492 → 293,046 | +8.3% | 3.295 → 2.997 | 3.711 → 3.361 | 512 / 0 | `a817e833dd81da75` |
| Deep | 12,522 → 15,907 | +27.0% | 80.213 → 63.402 | 83.605 → 66.391 | 512 / 0 | `17090361c775bb25` |
| Single-cycle | 2,318 → 2,315 | −0.1% | 138.759 → 137.933 | 882.318 → 874.255 | 771 / 128 | `73171884c0465aad` |
| Register-file plugin | 3,027 → 3,036 | +0.3% | 114.389 → 115.186 | 322.516 → 321.352 | 1,536 / 512 | `a2689cffb70ff825` |

Stage 5 measured 2,317 CPU ticks/s and attributed 70.5% of exclusive stable-settle samples to wasmi dispatch. Stage 12 measured 2,290 CPU ticks/s with ABI 4 wasmi. Those historical numbers give context. The same-binary pairs above determine the decision.

| Workload | Cold open (ms) | Plan preparation including JIT (ms) | Of which native compilation (ms) | First settle (ms) |
| --- | ---: | ---: | ---: | ---: |
| Wide | 5.75 → 15.75 | 3.99 → 13.81 | 9.46 | 0.265 → 0.260 |
| Deep | 2.16 → 4.85 | 0.96 → 3.80 | 2.88 | 0.166 → 0.141 |
| Single-cycle | 26.02 → 27.09 | 5.43 → 6.40 | 1.00 | 1.929 → 1.923 |
| Plugin | 9.46 → 9.10 | 0.87 → 0.98 | 0 | 0.695 → 0.684 |

## Evaluator cost and coverage

The isolated measurement replays the fixture through the compiled evaluator, including boundary reads and changed-drive handling. Outside the timer it hashes every compiled drive after every input change.

| Cached evaluator | Iterations/s | Speedup | Iteration p50 / p95 interpreted (µs) | Iteration p50 / p95 native (µs) | All-drive digest |
| --- | ---: | ---: | ---: | ---: | --- |
| Wide | 479,935 → 562,647 | 1.17× | 2.064 / 2.167 | 1.791 / 1.857 | `f65c5837f0c2bcc5` |
| Deep | 41,705 → 120,045 | 2.88× | 24.457 / 24.956 | 8.438 / 8.501 | `3cfd04ae11afa725` |

The cold plan counts include all instantiated circuits. The instruction denominator excludes folded constants. The component denominator includes clocks, pins, probes, state and plugin boundaries. Time shares are wall-clock measurements from separate profiling processes with the same trace. The region timer includes mask packing and changed-output scatter. These nested measurements carry timer overhead and should not be added together.

| Workload | Native nodes / components | Native / plan instructions | Regions / distinct kernels | Native share of executed instructions | Interpreter evaluator / settle | Native evaluator / settle | Native regions / settle |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Wide | 64 / 320 | 64 / 64 | 64 / 64 | 100% | 61.16% | 58.24% | 8.62% |
| Deep | 318 / 320 | 318 / 318 | 3 / 2 | 100% | 31.38% | 13.24% | 12.93% |
| Single-cycle | 13 / 961 | 13 / 160 | 13 / 5 | 7.00% | 6.82% | 6.92% | 0.27% |
| Plugin | 0 / 7 | 0 / 0 | 0 / 0 | No dynamic instructions | 0.05% | 0.04% | 0% |

For the CPU, all pure evaluator invocations together took 29.54 → 30.13 µs per tick in the profiling runs. The full stable settle includes plugin evaluation, state and network resolution. Even eliminating the measured interpreter evaluator time entirely would yield only about 7.3% on this trace with all other work held constant. This upper bound depends on the profiling denominator and overhead.

## Execution and safety boundary

- Eligible instructions are connected, unnegated-input NOT/Buffer chains of up to 128 gates and 1–32-bit adders with direct or constant inputs. Blocks materialize every internal drive. Four-state values use ones, unknown and error masks. Other instructions use the existing evaluator. The stable plan rejects unresolved feedback or width conflicts before native evaluation. Network resolution, state sampling, memory, plugin calls and event stepping retain their original execution paths.
- Cranelift 0.123.20 emits straight-line integer code with fixed frame offsets. The independent spike crate owns anonymous writable memory, seals it RX and calls it through one documented unsafe System V entry on Linux x86-64 with 64-bit pointers. The frame is exclusively borrowed and length-checked. The compiler verifier, bounded IR, native code generator and OS mapping protections form the trust boundary. Production crates retain their unsafe prohibition.
- A benchmark scope shares up to 128 normalized region keys across plans and resets. Keys include widths, gate sequence and constant masks. Semantic version and baseline target ABI/ISA are fixed by the running binary. Instances own separate scratch frames. A cache hit reuses immutable code. Unsupported targets, exhausted cache capacity and returned compile or executable-allocation errors use the interpreter. Tests inject allocation refusal and verify that dropping the last owner releases cached code. General process-wide allocator exhaustion was not induced.
- Verification compares all drive slots and changed outputs with the interpreter. Independent simulators compare every resolved net, net membership, diagnostics and clocks throughout the CPU and control traces, including all instantiated subcircuits and a subsequent event step. Four-state bus and carry tests also use the settled event engine as an oracle. All four workload digests, ticks, steps and work counts matched, with zero diagnostics.

Default desktop, Agent, simulator and benchmark dependency trees exclude the spike and Cranelift. The experiment has no persistent executable cache, packaged executable, engine preference or file-format changes. Native execution was measured only on Linux x86-64.

The all-feature workspace run passed 953 tests with zero failures. The existing manual GUI snapshot benchmark remained ignored. Formatting, all-target checks, Clippy with warnings denied, the default desktop/Agent/bench release builds and the experimental bench release build passed. Each final release verification replayed three warmup traces and 20 measured traces per workload.
