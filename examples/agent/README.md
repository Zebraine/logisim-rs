# Agent showcase

Open the native `.lrs` files in the desktop or headless Agent. The matching legacy
XML files are the original inputs to the conversion.

| Native project | Legacy source | Verified behavior |
| --- | --- | --- |
| [counter.lrs](counter.lrs) | [counter.circ](counter.circ) | After six clock ticks, the four-bit Count probe reads 3 (`0011`). |
| [adder.lrs](adder.lrs) | [adder.circ](adder.circ) | Set A to 5 and B to 3. The Sum probe reads 8 (`1000`). |
| [register-file.lrs](register-file.lrs) | [register-file.circ](register-file.circ) | Set Read address A to 1 and send the course plugin's `write` action with argument `1 DEADBEEF`. Read A returns `0xDEADBEEF`. |

Conversion preserves the persistent IDs of circuits, components and wires. Use
`agent_id` from a project snapshot to refer to the same object after reopening.
Keep the register-file project in its package layout so
`../../plugins/course/course.wasm` resolves to the bundled module.

## Open and export

From the unpacked Linux package directory:

```sh
./logisim-rs examples/agent/counter.lrs
./logisim-agent cli
```

In the running Agent, send the first line and substitute its returned
`data.session_id` for `ID` in the following lines. The export creates a
separate XML file. Choose a fresh output name.

```jsonl
{"tool":"project_open","arguments":{"path":"examples/agent/adder.lrs"}}
{"tool":"project_snapshot","arguments":{"session":"ID"}}
{"tool":"project_convert","arguments":{"session":"ID","output":"adder-copy.circ"}}
{"tool":"project_close","arguments":{"session":"ID"}}
```

In the desktop, new projects and **Save As** default to `.lrs`. Direct **Save**
on an opened `.circ` keeps its XML binding. Use **File → Export .circ…** for
a separate legacy copy. The converter reports properties XML cannot preserve.

## Rebuild from source

From the repository root:

```sh
cargo build --release --locked --offline -p logisim-agent
python3 examples/agent/build_showcase.py
```

The builder uses the public Agent CLI to convert each legacy source, reopen the
native output and check the probe values above. To also export three separate
legacy copies, run the command below with a destination containing no existing
showcase files. The Agent rebases plugin paths for that destination.

```sh
python3 examples/agent/build_showcase.py --export-legacy /tmp/logisim-showcase-export
```
