# Single-thread evaluator measurements

Baseline `be1ccb0`, Linux x86-64, i5-12600KF, Rust 1.99.0. Use the release bench with `--no-default-features --locked --offline` for both binaries. This keeps plugin timing instrumentation off. The engine is wasmi.

| Workload and scope | Largest exclusive native CPU sample shares | Callgrind observation |
| --- | --- | --- |
| Wide, stable settle | Cached evaluator 48.4%, resolution update 14.9%, bit resolution 14.1% | 2,057 evaluator calls, 24,842 self instructions/call |
| Deep, stable settle | Output materialization 22.2%, resolution update 21.3%, batch commit 9.9% | 651,264 incremental output writes plus 1,590 full materialization writes |
| Single-cycle, stable settle | wasmi dispatch 70.5%, resolution update 3.4%, cached evaluator 2.9% | 18,882 instance evaluator calls within the fixed four-trace process |
| Single-cycle, event advance | wasmi dispatch 85.3%, fuel consumption 1.3% | 131 whole-project advances, including initialization and reset |

The wide fixture changes one of 64 independent inputs per iteration. Its evaluator validates every boundary drive, including unchanged drives. The stable adapter searches the committed drive tree by port on each read. Test binding those ports to the existing instance-local resolution slots when preparing the plan. Keep the public evaluator's port-based API, validation, consumer queue and materialization order.

The native figures come from 1 kHz gperftools CPU samples with the named project operation on the stack. Sample counts in scope were 730, 4,236, 4,380 and 1,748 respectively. Inlined work is charged to the containing symbol. Callgrind counts come from separate runs collecting only inside `ProjectSimulator::settle_compiled` or `ProjectSimulator::advance`. The stable workload uses three warmup traces and one measured trace under Callgrind; the event workload uses the CPU event-reference test. These counts include the initial settle and reset boundaries in scope. Timed comparisons use native, uninstrumented runs.

To reproduce the stable counts, build the bench as above and run, for example:

```sh
valgrind --tool=callgrind --collect-atstart=no \
  '--toggle-collect=*ProjectSimulator*::settle_compiled' \
  --callgrind-out-file=/tmp/logisim-wide.cg target/release/logisim-bench wide --runs 1
callgrind_annotate --auto=no --inclusive=yes --tree=calling /tmp/logisim-wide.cg
```

## Paired release results

The candidate binds stable boundary ports to committed driver slots once per topology. Baseline and candidate ran on Linux CPU 15, an E-core with no SMT sibling, under the powersave governor. Each pair used fresh processes on the same core, alternating baseline-first and candidate-first. Both binaries ran three warmup traces and 20 measured traces per process. Each table entry shows the median of the corresponding process statistics. Throughput includes the input change or tick and all settles for that iteration.

| Workload | Pairs | Iterations/s before → after | Change | Settle p50 before → after (ms) | Settle p95 before → after (ms) | Four-state digest |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Wide | 4 | 233,582 → 277,984 | +19.0% | 0.003898 → 0.003212 | 0.004238 → 0.003515 | `a817e833dd81da75` |
| Deep | 4 | 11,671 → 12,678 | +8.6% | 0.085669 → 0.078397 | 0.089159 → 0.081852 | `17090361c775bb25` |
| Single-cycle | 10 | 2,386 → 2,317 | −2.9% | 0.136896 → 0.140286 | 0.865119 → 0.880248 | `73171884c0465aad` |
| Plugin | 10 | 3,161 → 3,119 | −1.3% | 0.110570 → 0.111661 | 0.316362 → 0.320967 | `a2689cffb70ff825` |

The clocked workloads had four initial pairs and six follow-up pairs. The plugin's first four pairs showed a 5.6% increase in total cold time. A dedicated cold check used 32 further alternating pairs with one first-settle observation per fresh process. It gave 8.935 → 9.137 ms for total cold time and 0.663 → 0.690 ms for first settle, increases of 2.3% and 4.1%. OS file pages were warm. The plugin's complete hot iteration p50 varied across processes. The medians across the ten processes moved from 0.250 to 0.265 ms, with a median within-pair change of +3.1% and three pairs slower by over 5%.

| Workload | Total cold before → after (ms) | Plan preparation before → after (ms) | First settle before → after (ms) |
| --- | ---: | ---: | ---: |
| Wide | 6.342 → 6.142 | 4.280 → 4.231 | 0.274 → 0.261 |
| Deep | 2.115 → 2.085 | 0.963 → 0.928 | 0.171 → 0.150 |
| Single-cycle | 25.628 → 25.755 | 5.268 → 5.322 | 1.923 → 1.910 |
| Plugin (32 dedicated cold pairs) | 8.935 → 9.137 | 0.856 → 0.871 | 0.663 → 0.690 |

All four wide pairs cleared the 10% throughput gate (+14.1% to +25.8%). The scoped wide Callgrind run recorded 88,356,731 → 70,939,602 instructions. Each build executed the same logical work, with matching step counts, ticks, diagnostics and digests. The CPU and plugin workloads showed small throughput losses. Stage 0 used an earlier build and a different process order. The decision uses the paired Stage 4 baseline.

Keep the indexed boundary reads. End this round of single-thread optimization here. More changes to scheduling, queues or allocation would need a representative target with a measured bottleneck and a separate comparison.
