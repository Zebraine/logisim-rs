#!/usr/bin/env python3
"""Build the agent showcase circuits through the headless logisim-agent.

The script deliberately uses the public CLI protocol only. It writes the three
.circ files next to itself, then reopens each file and prints a short result
summary. Run it from the repository after building logisim-agent:

    cargo build --release --locked --offline -p logisim-agent
    python3 examples/agent/build_showcase.py
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
AGENT = ROOT / "target" / "release" / "logisim-agent"

SEED = """<project version="1.0" source="logisim-rs-plugin-v1">
  <main name="main"/>
  <circuit name="main"/>
</project>
"""


class Agent:
    def __init__(self) -> None:
        self.path_for_session: dict[str, Path] = {}
        self.process = subprocess.Popen(
            [str(AGENT), "cli"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd=ROOT,
        )

    def send(self, value: object) -> dict:
        assert self.process.stdin is not None
        self.process.stdin.write(json.dumps(value, ensure_ascii=False) + "\n")
        self.process.stdin.flush()
        assert self.process.stdout is not None
        line = self.process.stdout.readline()
        if not line:
            error = self.process.stderr.read() if self.process.stderr else ""
            raise RuntimeError(f"agent closed without a response: {error}")
        reply = json.loads(line)
        if not reply.get("ok"):
            raise RuntimeError(json.dumps(reply, ensure_ascii=False, indent=2))
        return reply["data"]

    def open(self, path: Path) -> str:
        session = self.send(["project", "open", "--path", str(path)])["session_id"]
        self.path_for_session[session] = path
        return session

    def patch(self, session: str, ops: list[dict]) -> dict:
        return self.send(
            {
                "tool": "project_patch",
                "arguments": {"session": session, "dry_run": False, "ops": ops},
            }
        )

    def save(self, session: str) -> dict:
        result = self.send(
            ["project", "save", "--session", session, "--no-backup"]
        )
        # The public CLI keeps lock sidecars while a session is alive; the
        # showcase directory should contain only portable examples.
        for suffix in (".bak", ".lock"):
            path = self.path_for_session.get(session)
            if path is not None:
                Path(str(path) + suffix).unlink(missing_ok=True)
        return result

    def close(self, session: str) -> None:
        self.send(["project", "close", "--session", session])

    def finish(self) -> None:
        if self.process.stdin:
            self.process.stdin.close()
        code = self.process.wait()
        if code:
            error = self.process.stderr.read() if self.process.stderr else ""
            raise RuntimeError(f"logisim-agent exited with {code}: {error}")


def prepare(name: str) -> tuple[Agent, str, Path]:
    path = HERE / f"{name}.circ"
    path.write_text(SEED)
    agent = Agent()
    session = agent.open(path)
    return agent, session, path


def wire(circuit: int, a: tuple[int, int], b: tuple[int, int]) -> dict:
    return {"op": "add_wire", "circuit": circuit, "from": list(a), "to": list(b)}


def component(circuit: int, kind: str, at: tuple[int, int], **props: object) -> dict:
    return {
        "op": "add_component",
        "circuit": circuit,
        "kind": kind,
        "position": list(at),
        "properties": props,
    }


def build_counter() -> str:
    agent, session, path = prepare("counter")
    change = agent.patch(
        session,
        [
            {"op": "rename_circuit", "id": 1, "name": "Counter"},
            component(1, "Clock", (100, 100), label="Clock"),
            component(
                1,
                "Counter",
                (300, 100),
                bits=4,
                counter_max=15,
                label="Count",
            ),
            component(1, "Probe", (450, 100), radix="hex", label="Count"),
            component(1, "Power", (180, 300)),
            component(1, "Ground", (200, 20)),
            component(1, "Ground", (350, 200)),
            component(1, "Ground", (100, 320)),
            wire(1, (100, 100), (100, 120)),
            wire(1, (100, 120), (280, 120)),
            wire(1, (300, 100), (450, 100)),
            wire(1, (180, 300), (180, 80)),
            wire(1, (180, 80), (280, 80)),
            wire(1, (200, 20), (270, 20)),
            wire(1, (270, 20), (270, 90)),
            wire(1, (350, 200), (290, 200)),
            wire(1, (290, 200), (290, 120)),
            wire(1, (290, 120), (290, 80)),
            wire(1, (100, 320), (270, 320)),
            wire(1, (270, 320), (270, 110)),
        ],
    )
    agent.send(["simulation", "open", "--session", session])
    for _ in range(6):
        agent.send(["simulation", "tick", "--session", session])
    state = agent.send(["simulation", "state", "--session", session])
    probe = next(p for p in state["probes"] if p["label"] == "Count")
    text = (
        f"- `counter.circ`: {change['changed']} edit, 6 ticks, "
        f"Count={probe['value']['number']} (hex {probe['value']['binary']}), "
        f"diagnostics={len(state['diagnostics'])}."
    )
    agent.save(session)
    agent.close(session)
    agent.finish()
    reopened = Agent()
    session = reopened.open(path)
    snapshot = reopened.send(["project", "snapshot", "--session", session])
    text += f" Reopened: {len(snapshot['circuits'])} circuit(s), {len(snapshot['circuits'][0]['components'])} components, {len(snapshot['circuits'][0]['wires'])} wires."
    reopened.close(session)
    reopened.finish()
    return text


def build_adder() -> str:
    agent, session, path = prepare("adder")
    agent.patch(
        session,
        [
            {"op": "rename_circuit", "id": 1, "name": "Adder"},
            component(1, "Pin", (100, 60), bits=4, output=False, label="A"),
            component(1, "Pin", (100, 140), bits=4, output=False, label="B"),
            component(1, "Adder", (300, 100), bits=4),
            component(1, "Probe", (450, 100), radix="hex", label="Sum"),
            wire(1, (100, 60), (260, 60)),
            wire(1, (260, 60), (260, 90)),
            wire(1, (100, 140), (260, 140)),
            wire(1, (260, 140), (260, 110)),
            wire(1, (300, 100), (450, 100)),
        ],
    )
    agent.send(["simulation", "open", "--session", session])
    state = agent.send(["simulation", "state", "--session", session])
    ids = {p["label"]: p["id"] for p in state["pins"]}
    agent.send(
        [
            "simulation",
            "set-input",
            "--session",
            session,
            "--id",
            str(ids["A"]),
            "--value",
            "5",
        ]
    )
    agent.send(
        [
            "simulation",
            "set-input",
            "--session",
            session,
            "--id",
            str(ids["B"]),
            "--value",
            "3",
        ]
    )
    state = agent.send(["simulation", "set-input", "--session", session, "--id", str(ids["A"]), "--value", "5"])
    probe = next(p for p in state["state"]["probes"] if p["label"] == "Sum")
    text = (
        f"- `adder.circ`: A=5, B=3, Sum={probe['value']['number']} "
        f"(hex {probe['value']['binary']}), diagnostics={len(state['state']['diagnostics'])}."
    )
    agent.save(session)
    agent.close(session)
    agent.finish()
    reopened = Agent()
    session = reopened.open(path)
    snapshot = reopened.send(["project", "snapshot", "--session", session])
    text += f" Reopened: {len(snapshot['circuits'][0]['components'])} components."
    reopened.close(session)
    reopened.finish()
    return text


def build_register_file() -> str:
    agent, session, path = prepare("register-file")
    source = {
        "plugin": "org.logisim-rs.course",
        "version": "1.0.0",
        "path": "../../plugins/course/course.wasm",
        "fallback": "",
    }
    change = agent.patch(
        session,
        [
            {"op": "rename_circuit", "id": 1, "name": "RegisterFile"},
            {
                "op": "add_plugin",
                "circuit": 1,
                "position": [350, 300],
                "tool": "register-file",
                "source": source,
                "properties": {"trigger": "rising"},
                "label": "Register File",
            },
            component(1, "Pin", (100, 290), bits=32, output=False, label="W"),
            component(1, "Pin", (100, 340), bits=5, output=False, label="Write address"),
            component(1, "Pin", (100, 410), bits=5, output=False, label="Read address A"),
            component(1, "Probe", (500, 250), radix="hex", label="Read A"),
            component(1, "Power", (100, 440)),
            component(1, "Clock", (100, 380), label="Clock"),
            wire(1, (100, 290), (190, 290)),
            wire(1, (100, 340), (260, 340)),
            wire(1, (260, 340), (260, 390)),
            wire(1, (100, 410), (300, 410)),
            wire(1, (300, 410), (300, 390)),
            wire(1, (100, 440), (230, 440)),
            wire(1, (230, 440), (230, 390)),
            wire(1, (100, 380), (190, 380)),
            wire(1, (350, 250), (500, 250)),
        ],
    )
    agent.send(["simulation", "open", "--session", session])
    state = agent.send(["simulation", "state", "--session", session])
    ids = {c["label"]: c["id"] for c in state["components"]}
    operations = [
        ("Write address", 1),
        ("Read address A", 1),
        ("W", 0xDEADBEEF),
    ]
    for label, value in operations:
        agent.send(
            [
                "simulation",
                "set-input",
                "--session",
                session,
                "--id",
                str(ids[label]),
                "--value",
                str(value),
            ]
        )
    action = agent.send(
        [
            "simulation",
            "plugin-action",
            "--session",
            session,
            "--id",
            str(ids["Register File"]),
            "--action",
            "write",
            "--argument",
            "1 DEADBEEF",
        ]
    )
    probe = next(p for p in action["state"]["probes"] if p["label"] == "Read A")
    text = (
        f"- `register-file.circ`: {change['changed']} edit, plugin action write, "
        f"Read A={probe['value']['number'] if probe['value']['number'] is not None else 'X'} "
        f"(hex {probe['value']['binary']}), errors={len(action['state']['diagnostics'])}."
    )
    agent.save(session)
    agent.close(session)
    agent.finish()
    reopened = Agent()
    session = reopened.open(path)
    snapshot = reopened.send(["project", "snapshot", "--session", session])
    plugin = snapshot["circuits"][0]["components"][0]
    text += (
        f" Reopened: plugin={plugin['plugin']['id']}@{plugin['plugin']['version']}, "
        f"sources={len(snapshot['plugin_sources'])}."
    )
    reopened.close(session)
    reopened.finish()
    return text


def main() -> int:
    if not AGENT.exists():
        print(f"missing {AGENT}; build logisim-agent first", file=sys.stderr)
        return 2
    print("Built by logisim-agent:")
    for build in (build_counter, build_adder, build_register_file):
        print(build())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
