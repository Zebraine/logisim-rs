"""Linux release module-cache experiment. Emit JSON; caches live only in private temp dirs."""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

from abi4 import fixtures

REPO = Path(__file__).resolve().parents[3]
TRAJECTORIES = {"plugin": ("a2689cffb70ff825", 512, 1536),
                "single-cycle": ("73171884c0465aad", 128, 771)}


def size(directory):
    return sum(p.stat().st_size for p in directory.glob("slot-*"))


def measure(bench, name, root, mode, runs, cpu, fixtures):
    args = ["taskset", "-c", str(cpu), bench, "--wasm-engine", "wasmtime", "engine-study",
            "opens", name, mode, "--runs", str(runs)]
    env = dict(os.environ, LOGISIM_BENCH_PRIVATE_CACHE=str(root))
    process = subprocess.run(args, cwd=fixtures, env=env, stdout=subprocess.PIPE, check=True)
    report = json.loads(process.stdout)
    rows = report["opens"]
    assert report["peak_rss_kib"] > 0
    for row in rows:
        assert (row["digest"], row["ticks"], row["steps"]) == TRAJECTORIES[name]
        assert row["diagnostics"] == 0
        assert (row["plugin_abi"], row["wasm_engine"]) == (4, "wasmtime")
    return {"opens": rows, "max_rss_kib": report["peak_rss_kib"], "cache_bytes": size(root)}


def one_trial(args, name, trial):
    with tempfile.TemporaryDirectory(prefix="logisim-module-cache-") as temp:
        root = Path(temp) / "cache"
        def run(mode="module-cache"):
            return measure(args.bench, name, root, mode, args.reopens, args.cpu, args.fixtures)

        result = {}
        for mode in (["none", "module-cache"] if trial % 2 == 0 else ["module-cache", "none"]):
            result["uncached" if mode == "none" else "empty"] = run(mode)
        result["second"], result["third"] = run(), run()
        assert result["empty"]["opens"][0]["misses_so_far"] == 1
        for key in ("empty", "second", "third"):
            rows = result[key]["opens"]
            assert rows[-1]["hits_so_far"] == len(rows) - (key == "empty")
            assert all(r["cache_warning"] is None for r in rows)

        record, = root.glob("slot-*")
        record.write_bytes(record.read_bytes()[:12])
        result["corrupt"] = run()
        assert result["corrupt"]["opens"][0]["cache_warning"]
        assert result["corrupt"]["opens"][-1]["hits_so_far"] == args.reopens
        root.chmod(0o500)
        try:
            result["unwritable"] = run()
            assert all(r["cache_warning"] and r["hits_so_far"] == 0
                       for r in result["unwritable"]["opens"])
        finally:
            root.chmod(0o700)
        record.unlink()
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as workers:
            futures = [workers.submit(measure, args.bench, name, root, "module-cache",
                                      args.reopens, cpu, args.fixtures) for cpu in [args.cpu, args.peer_cpu]]
            result["concurrent"] = [f.result() for f in futures]
        result["after_concurrent"] = run()
        assert result["after_concurrent"]["opens"][0]["hits_so_far"] == 1
        remaining = sorted(root.iterdir())
        assert remaining == [record], f"Unexpected cache contents after worker exit: {remaining}"
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bench", type=Path, default=REPO/"target/release/logisim-bench")
    parser.add_argument("--abi4", type=Path, default=REPO/"target/wasm32-unknown-unknown/release/logisim_course_plugin.wasm",
                        help="Course module built with execution-abi4")
    parser.add_argument("--cpu", required=True, type=int)
    parser.add_argument("--peer-cpu", required=True, type=int)
    parser.add_argument("--trials", type=int, default=5)
    parser.add_argument("--reopens", type=int, default=3)
    args = parser.parse_args()
    args.bench = args.bench.resolve()
    if args.trials < 1 or args.reopens < 1:
        parser.error("trials and reopens must be positive")
    with tempfile.TemporaryDirectory(prefix="logisim-abi4-fixtures-") as projects:
        args.fixtures = Path(projects)
        fixtures(args.fixtures, args.abi4)
        collect(args)


def collect(args):
    report = {"platform": platform.platform(), "cpu": args.cpu, "peer_cpu": args.peer_cpu,
              "bench_sha256": hashlib.sha256(args.bench.read_bytes()).hexdigest(),
              "wasm_sha256": hashlib.sha256((args.fixtures/"plugins/course/course.wasm").read_bytes()).hexdigest(),
              "workloads": {name: [] for name in TRAJECTORIES}}
    for trial in range(args.trials):
        for name, results in report["workloads"].items():
            results.append(one_trial(args, name, trial))
            print(f"{name} trial {trial+1} passed", file=sys.stderr, flush=True)
    print(json.dumps(report))


if __name__ == "__main__":
    main()
