"""Linux release comparison. Supply two separately built bench binaries and a new output directory."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile


def fuel_matrix(cpu, directory):
    results = {}
    cases = [(n, p, m * 10_000_000)
             for n in (1000, 10_000, 100_000, 1_000_000, 2_000_000, 3_600_000, None)
             for p in ("outputs", "transition") for m in (1, 2, 4, 8)]
    for engine, features in (("wasmi", "engine-study"), ("wasmtime", "engine-study,compare-wasmtime")):
        command = ["cargo", "test", "--release", "--locked", "--offline", "-p", "logisim-plugin",
                   "--features", features, "phase_fuel_contract_matrix", "--", "--nocapture"]
        output = subprocess.check_output(["taskset", "-c", str(cpu), *command], text=True)
        # libtest may prefix the first line with the name of the running test.
        rows = [json.loads(line.split("FUEL ", 1)[1]) for line in output.splitlines()
                if "FUEL {" in line]
        results[engine] = {(r["iterations"], r["phase"], r["budget"]): r for r in rows}
        if set(results[engine]) != set(cases) or len(rows) != len(cases):
            raise ValueError(f"Incomplete {engine} fuel matrix")
        (directory / f"fuel-{engine}.json").write_text(json.dumps(rows, indent=2) + "\n")
    for case in cases:
        left, right = (results[engine][case] for engine in ("wasmi", "wasmtime"))
        if left["sha256"] != right["sha256"]:
            raise ValueError(f"Different Wasm for {case}")
        if left["error"] is None and right["error"] is None and left["phase_return"] != right["phase_return"]:
            raise ValueError(f"Different successful phase return for {case}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wasmi", required=True, type=Path)
    parser.add_argument("--wasmtime", required=True, type=Path)
    parser.add_argument("--cpu", required=True, type=int)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--disk-only", action="store_true", help="measure fresh and cross-process disk opens only")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    binaries = {"wasmi": args.wasmi.resolve(), "wasmtime": args.wasmtime.resolve()}
    fixtures = {
        "plugin": ("examples/agent/register-file.circ", "a2689cffb70ff825", 512),
        "single-cycle": ("examples/course/single-cycle.circ", "73171884c0465aad", 128),
    }
    assets = [v[0] for v in fixtures.values()] + ["plugins/course/course.wasm", *binaries.values()]
    def identify(*command):
        return subprocess.check_output(command, text=True).strip()
    def write(name, value):
        (args.out / (name + ".json")).write_text(json.dumps(value, indent=2) + "\n")
    write("environment", {
        "commit": identify("git", "rev-parse", "HEAD"), "cpu": args.cpu, "os": platform.platform(),
        "toolchain": identify("rustc", "-vV"), "cpu_info": identify("lscpu"),
        "sha256": {str(p): hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in assets},
    })

    tree = identify("cargo", "tree", "--locked", "--offline", "-e", "normal,build", "-p",
                    "logisim-rs", "-p", "logisim-agent", "-p", "logisim-bench", "-p", "logisim-plugin")
    if "wasmtime v" in tree or "wasmtime-internal-" in tree:
        raise ValueError("Default build depends on Wasmtime")
    (args.out / "default-tree.txt").write_text(tree + "\n")
    if not args.disk_only:
        fuel_matrix(args.cpu, args.out)
    references = {}
    def measure(engine, action, workload, mode, rep, directory=None, populated=False):
        env = os.environ.copy()
        if directory is not None:
            env["LOGISIM_BENCH_PRIVATE_CACHE"] = str(directory)
        report = json.loads(subprocess.check_output(
            ["taskset", "-c", str(args.cpu), binaries[engine], "engine-study", action,
             workload, str(mode), "--runs", "20"], env=env))
        label = mode if not populated else "disk-reopen"
        write(f"{engine}-{action}-{workload}-{label}-{rep}", report)
        records = report["opens"] if action == "opens" else [report]
        for row in records:
            if (row["digest"], row["ticks"]) != fixtures[workload][1:]:
                raise ValueError(f"{workload} trace mismatch")
        if action == "trace":
            if report["wasm_engine"] != {"wasmi": "wasmi-1.1.0", "wasmtime": "wasmtime-36.0.20"}[engine]:
                raise ValueError("Wrong experimental binary")
            signature = [report[k] for k in ("digest", "steps", "ticks", "work_per_run", "diagnostics")]
            if references.setdefault(workload, signature) != signature:
                raise ValueError("Engine or fuel budget changed the work or visible state")
            fuel = report["plugin_phases"]["hot"]
            for phase in ("outputs", "transition"):
                if fuel[phase]["fuel"]["dispatches"] != fuel[phase]["calls"]:
                    raise ValueError(f"Missing real {phase} fuel readings")
        if mode == "private-disk":
            misses = 0 if populated else 1
            if (report["cache_error"] or records[-1]["misses_so_far"] != misses
                    or records[-1]["hits_so_far"] != 21 - misses):
                raise ValueError(f"Unexpected native cache result: {records[-1]}")
        print(engine, action, workload, label, rep, flush=True)

    with tempfile.TemporaryDirectory(prefix="logisim-engine-study-") as scratch:
        for workload in fixtures:
            for rep in range(1, 4):
                directory = Path(scratch) / f"{workload}-{rep}"
                directory.mkdir(mode=0o700)
                for engine in (list(binaries) if rep % 2 else list(reversed(binaries))):
                    if not args.disk_only:
                        for multiplier in ([1] if engine == "wasmi" else [1, 2, 4, 8]):
                            measure(engine, "trace", workload, multiplier, rep)
                        for policy in ("module", "engine", "none"):
                            measure(engine, "opens", workload, policy, rep)
                    if engine == "wasmtime":
                        measure(engine, "opens", workload, "private-disk", rep, directory)
                        measure(engine, "opens", workload, "private-disk", rep, directory, populated=True)


if __name__ == "__main__":
    main()
