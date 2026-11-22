"""Run the Stage 16 Linux matrix from the workspace root. JSONL goes to stdout."""
import argparse
import json
import os
from pathlib import Path
import resource
import subprocess
import tempfile
import time


def run(binary, cpus, engine, name, threads, cache, runs, warm=False):
    command = ["taskset", "-c", cpus, binary, "--wasm-engine", engine]
    cached = engine == "wasmtime" and name in ("single-cycle", "plugin")
    if cached:
        command += ["engine-study", "cached-trace", name, "module-cache"]
    else:
        command.append(name)
    env = dict(os.environ, LOGISIM_BENCH_DATAFLOW_THREADS=str(threads),
               LOGISIM_BENCH_PRIVATE_CACHE=cache)
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.perf_counter()
    result = json.loads(subprocess.check_output(command + ["--runs", str(runs)], env=env,
                                                timeout=120))
    elapsed = time.perf_counter() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    result["process_cpu_percent"] = 100 * (
        after.ru_utime - before.ru_utime + after.ru_stime - before.ru_stime
    ) / elapsed
    assert (result["threads"], result["runs"], result["warmup"]) == (threads, runs, 3)
    if name in ("single-cycle", "plugin"):
        assert result["wasm_engine"].startswith(engine + "-") and result["plugin_abi"] == 4
    if cached:
        assert result["cache_warning"] is None
        if not warm:
            assert (result["cache_hits"], result["cache_misses"]) == (1, 0)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for binary in ("baseline", "timed", "traced"):
        parser.add_argument("--" + binary, required=True, type=Path)
    parser.add_argument("--cpus", required=True)
    parser.add_argument("--trials", type=int, default=3)
    args = parser.parse_args()
    refs = {}
    with tempfile.TemporaryDirectory(prefix="logisim-dataflow-") as cache:
        run(args.baseline, args.cpus, "wasmtime", "plugin", 1, cache, 1, warm=True)
        for trial in range(args.trials):
            for name in ("wide", "deep", "single-cycle", "plugin"):
                for engine in ("wasmi", "wasmtime"):
                    order = [0, 1, 2, 4]
                    offset = trial % len(order)
                    for n in order[offset:] + order[:offset]:
                        row = run(args.baseline if n == 0 else args.timed, args.cpus,
                                  engine, name, max(1, n), cache, 20)
                        check = [row[k] for k in ("digest", "ticks", "steps", "diagnostics",
                                                  "work_per_run")]
                        assert check == refs.setdefault(name, check)
                        print(json.dumps(dict(trial=trial, variant=n, timing=row)), flush=True)
                    for n in (1, 2, 4):
                        row = run(args.traced, args.cpus, engine, name, n, cache, 1)
                        assert [row[k] for k in ("digest", "ticks", "steps", "diagnostics",
                                                 "work_per_run")] == refs[name]
                        print(json.dumps(dict(trial=trial, variant=n, engine=engine, workload=name,
                                              study=row["dataflow"])), flush=True)


if __name__ == "__main__":
    main()
