"""Compare shipped ABI 3 and explicit ABI 4 on unchanged project copies (Linux release)."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

REPO = Path(__file__).resolve().parents[3]
COURSE = Path("plugins/course/course.wasm")
CASES = {
    "plugin": ("examples/agent/register-file.circ", "a2689cffb70ff825", 512, 1536),
    "single-cycle": ("examples/course/single-cycle.circ", "73171884c0465aad", 128, 771),
}


def fixtures(root, wasm):
    for source in [case[0] for case in CASES.values()]:
        target = root / source
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(REPO / source, target)
    (root / COURSE).parent.mkdir(parents=True)
    shutil.copyfile(wasm, root / COURSE)


def run(args, root, name, abi, engine, cache=None):
    command = ["taskset", "-c", str(args.cpu), args.bench, "--wasm-engine", engine]
    env = os.environ.copy()
    if cache:
        command += ["engine-study", "cached-trace", name, "module-cache"]
        env["LOGISIM_BENCH_PRIVATE_CACHE"] = str(cache)
    else:
        command.append(name)
    result = subprocess.run(command + ["--runs", str(args.runs)], cwd=root, env=env,
                            stdout=subprocess.PIPE, check=True)
    row = json.loads(result.stdout)
    _, digest, ticks, steps = CASES[name]
    assert (row["digest"], row["ticks"], row["steps"], row["diagnostics"]) == (digest, ticks, steps, 0)
    assert (row["plugin_abi"], row["wasm_engine"].split("-")[0]) == (abi, engine)
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--abi4", type=Path, required=True, help="SDK-built course ABI 4 module")
    parser.add_argument("--bench", type=Path, default=REPO / "target/release/logisim-bench")
    parser.add_argument("--cpu", type=int, required=True)
    parser.add_argument("--trials", type=int, default=5)
    parser.add_argument("--runs", type=int, default=20)
    args = parser.parse_args()
    args.bench = args.bench.resolve()
    if args.trials < 1:
        parser.error("trials must be positive")
    report = {"sha256": {str(abi): hashlib.sha256(path.read_bytes()).hexdigest()
                         for abi, path in [(3, REPO / COURSE), (4, args.abi4)]},
              "cpu": args.cpu, "trials": []}
    with tempfile.TemporaryDirectory(prefix="logisim-abi4-") as scratch:
        base = Path(scratch)
        for abi, wasm in [(3, REPO / COURSE), (4, args.abi4)]:
            fixtures(base / str(abi), wasm)
        for trial in range(args.trials):
            for name in CASES:
                row = {"trial": trial + 1, "workload": name}
                for abi in ([3, 4] if trial % 2 == 0 else [4, 3]):
                    row[f"wasmi{abi}"] = run(args, base / str(abi), name, abi, "wasmi")
                cache = base / f"cache-{name}-{trial}"
                for state, hits in [("empty", 0), ("hit", 1)]:
                    result = run(args, base / "4", name, 4, "wasmtime", cache)
                    assert (result["cache_hits"], result["cache_misses"], result["cache_warning"]) == (hits, 1-hits, None)
                    row[state] = result
                report["trials"].append(row)
    print(json.dumps(report))


if __name__ == "__main__":
    main()
