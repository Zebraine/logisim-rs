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
