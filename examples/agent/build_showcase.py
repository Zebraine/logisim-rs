#!/usr/bin/env python3
"""Convert the legacy Agent showcase with the release Agent and verify its probe values."""

import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


HERE = Path(__file__).resolve().parent
AGENT = HERE.parents[1] / "target/release" / ("logisim-agent.exe" if os.name == "nt" else "logisim-agent")
NAMES = ("counter", "adder", "register-file")


def call(process, tool, **arguments):
    request = {"tool": tool, "arguments": arguments}
    process.stdin.write(json.dumps(request) + "\n")
    process.stdin.flush()
    line = process.stdout.readline()
    if not line:
        raise RuntimeError(f"Agent exited during {tool}")
    reply = json.loads(line)
    if not reply["ok"]:
        raise RuntimeError(f"{tool}: {reply['error']}")
    return reply["data"]


@contextmanager
def session(process, path):
    opened = call(process, "project_open", path=str(path))
    sid = opened["session_id"]
    try:
        if opened["partial"] or opened["warnings"]:
            raise ValueError(f"Incomplete import of {path}: {opened['warnings']}")
        yield sid
    finally:
        call(process, "project_close", session=sid)


def verify(process, sid, name):
    def run(tool, **args):
        return call(process, tool, session=sid, **args)

    circuit, = run("project_snapshot")["circuits"]
    run("simulation_open")
    state = run("simulation_state")
    pins = {pin["label"]: pin["id"] for pin in state["pins"]}
    if name == "counter":
        for _ in range(6):
            run("simulation_tick")
        label, expected = "Count", 3
    elif name == "adder":
        for pin, value in (("A", 5), ("B", 3)):
            run("simulation_set_input", id=pins[pin], value=value)
        label, expected = "Sum", 8
    else:
        for pin, value in (("Write address", 1), ("Read address A", 1), ("W", 0xDEADBEEF)):
            run("simulation_set_input", id=pins[pin], value=value)
        register = next(c for c in circuit["components"] if c["label"] == "Register File")
        run("simulation_plugin_action", agent_id=register["agent_id"], action="write", argument="1 DEADBEEF")
        label, expected = "Read A", 0xDEADBEEF

    state = run("simulation_state")
    probe = next(p for p in state["probes"] if p["label"] == label)
    if probe["value"]["number"] != expected or state["error"] or state["diagnostics"]:
        raise ValueError(f"{name}: {probe}, error={state['error']}, diagnostics={state['diagnostics']}")
    return f"{len(circuit['components'])} components, {len(circuit['wires'])} wires, {label}={expected:#x}"


def convert(process, source, destination):
    # Stage beside the destination so the codec resolves plugin paths against the final directory.
    with tempfile.NamedTemporaryFile(dir=destination.parent, suffix=destination.suffix, delete=False) as empty:
        staged = Path(empty.name)
    try:
        with session(process, source) as sid:
            result = call(process, "project_convert", session=sid, output=str(staged),
                          expected_output_sha256=hashlib.sha256(b"").hexdigest(), backup=False)
        with session(process, staged) as sid:
            summary = verify(process, sid, source.stem)
        shutil.copymode(source, staged)
        if destination.suffix == ".lrs":
            staged.replace(destination)
        else:
            # Explicit exports are create-only, including if another writer created the target.
            os.link(staged, destination)
        print(f"{destination} ({result['format']}): {summary}")
    finally:
        staged.unlink(missing_ok=True)
        Path(str(staged) + ".lock").unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--export-legacy", type=Path, metavar="DIR",
                        help="also export separate .circ copies to DIR")
    args = parser.parse_args()
    if args.export_legacy:
        directory = args.export_legacy.resolve()
        if any((directory / f"{name}.circ").exists() for name in NAMES):
            parser.error("Choose a directory without existing showcase .circ files.")
        directory.mkdir(parents=True, exist_ok=True)
    with subprocess.Popen([AGENT, "cli"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                          text=True, encoding="utf-8") as process:
        for name in NAMES:
            native = HERE / f"{name}.lrs"
            convert(process, native.with_suffix(".circ"), native)
            if args.export_legacy:
                convert(process, native, directory / f"{name}.circ")
        process.stdin.close()
        if process.wait():
            raise RuntimeError("Agent exited unsuccessfully")


if __name__ == "__main__":
    main()
