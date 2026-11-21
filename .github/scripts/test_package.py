"""Exercise the shipped Agent and showcase from a freshly extracted native archive."""

import argparse
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import tempfile


def check_archive(archive):
    root = Path(__file__).resolve().parents[2]
    # Use the same probe/input trajectories as the generator, with the Agent in the archive.
    showcase = runpy.run_path(str(root / "examples/agent/build_showcase.py"))
    names = showcase["NAMES"]
    session, call, verify = showcase["session"], showcase["call"], showcase["verify"]
    with tempfile.TemporaryDirectory(prefix="logisim-package-test-") as tmp:
        shutil.unpack_archive(archive, tmp)
        package, = Path(tmp).iterdir()
        examples = package / "examples/agent"
        projects = {f"{name}.{ext}" for name in names for ext in ("lrs", "circ")}
        entries = {file.name for file in examples.iterdir()}
        if entries != projects | {"README.md"}:
            raise ValueError(f"Unexpected showcase contents: {sorted(entries)}")
        for entry in entries:
            if (examples / entry).read_bytes() != (root / "examples/agent" / entry).read_bytes():
                raise ValueError(f"Packaged file differs from its source: {entry}")

        binary = package / ("logisim-agent.exe" if os.name == "nt" else "logisim-agent")
        with subprocess.Popen([binary, "cli"], cwd=package, stdin=subprocess.PIPE,
                              stdout=subprocess.PIPE, text=True, encoding="utf-8") as process:
            for name in names:
                native = examples / f"{name}.lrs"
                legacy_source = native.with_suffix(".circ")
                returned = examples / f"{name}.exported.circ"
                rebuilt = examples / f"{name}.reopened.lrs"
                with session(process, native) as sid:
                    summary = verify(process, sid, name)
                    call(process, "project_convert", session=sid, output=str(returned))
                for legacy in (legacy_source, returned):
                    with session(process, legacy) as sid:
                        if verify(process, sid, name) != summary:
                            raise ValueError(f"Simulation differs for {legacy.name}")
                        call(process, "project_convert", session=sid, output=str(rebuilt))
                    if rebuilt.read_bytes() != native.read_bytes():
                        raise ValueError(f"Native round trip differs for {legacy.name}")
                    rebuilt.unlink()
                before = set(examples.iterdir())
                showcase["convert"](process, legacy_source, native)
                if set(examples.iterdir()) != before:
                    raise ValueError(f"Builder left intermediate files for {name}")
                for path in (legacy_source, native):
                    if path.read_bytes() != (root / "examples/agent" / path.name).read_bytes():
                        raise ValueError(f"Regeneration changed {path.name}")
                print(f"{name}: both formats and explicit legacy export passed, {summary}")
            process.stdin.close()
            if process.wait():
                raise RuntimeError("Packaged Agent exited unsuccessfully")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    check_archive(parser.parse_args().archive.resolve())
