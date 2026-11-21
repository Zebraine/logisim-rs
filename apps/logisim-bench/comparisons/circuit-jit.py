"""Linux paired release experiment. Run from the workspace root; output stays local."""
import argparse
import json
import os
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/logisim-bench"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--core", type=int, required=True)
    parser.add_argument("--pairs", type=int, default=3)
    args = parser.parse_args()
    os.sched_setaffinity(0, {args.core})
    args.output.mkdir(parents=True, exist_ok=True)
    workloads = ("wide", "deep", "single-cycle", "plugin")

    def sample(mode, name, repetition, evaluator=False):
        scope = "evaluator" if evaluator else "project"
        cmd = [str(args.binary.resolve()), "--circuit-jit", mode]
        if evaluator:
            cmd.append("--evaluator-only")
        output = subprocess.check_output([*cmd, name, "--runs", "20"], text=True)
        result = json.loads(output)
        assert result["warmup"] == 3 and result["runs"] == 20
        (args.output / f"{scope}-{name}-{mode}-{repetition}.json").write_text(output)
        keys = ("drive_digest",) if evaluator else ("digest", "steps", "ticks", "diagnostics")
        return tuple(result[k] for k in keys)

    for repetition in range(1, args.pairs + 1):
        for name in workloads:
            for evaluator in ([False, True] if name in workloads[:2] else [False]):
                modes = ("interpreter", "native")
                if repetition % 2 == 0:
                    modes = modes[::-1]
                a, b = [sample(m, name, repetition, evaluator) for m in modes]
                assert a == b, (name, repetition, a, b)
    # Collect instrumented timings separately from all uninstrumented pairs.
    for repetition in range(1, args.pairs + 1):
        for name in workloads:
            a, b = [sample(m, name, repetition) for m in ("profile-interpreter", "profile-native")]
            assert a == b, (name, repetition, a, b)


if __name__ == "__main__":
    main()
