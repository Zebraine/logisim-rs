# Agent-built showcase circuits

These circuits were created and edited through the separate `logisim-agent`
service, using its public JSON CLI. The desktop GUI was not involved.

| File | Demonstrates |
| --- | --- |
| [`counter.circ`](counter.circ) | four-bit counter, clock wiring, explicit ticks, persistent IDs |
| [`adder.circ`](adder.circ) | input pins, an Adder bus, a probe, simulation input changes |
| [`register-file.circ`](register-file.circ) | course Wasm plugin instance, plugin action, register read-back |

## Verified results

- `counter.circ`: after six explicit clock ticks the `Count` probe is
  `3` (`0011`), with no diagnostics. The saved circuit has 7 components and
  12 wire segments.
- `adder.circ`: setting A to 5 and B to 3 produces `Sum = 8` (`1000`), with no
  diagnostics.
- `register-file.circ`: the course `register-file` plugin accepts
  `write "1 DEADBEEF"`; the `Read A` probe reads `0xDEADBEEF`
  (3735928559), with no plugin errors. The plugin source is stored as the
  relative path `../../plugins/course/course.wasm`.

Every saved file also contains optional `agent-id` attributes on circuits,
components and wires. These are the IDs the agent uses across processes; they
are document metadata, not simulation state.

## Reproduce

Build the headless agent, then run the builder from the repository root:

```sh
cargo build --release --locked --offline -p logisim-agent
python3 examples/agent/build_showcase.py
```

The builder opens a minimal `.circ`, applies semantic `project_patch` batches,
opens the simulation, drives inputs/ticks/plugin actions, saves, reopens, and
prints the result table above. It intentionally uses the same public CLI that
an external agent would use.

The files can also be opened in the desktop application:

```sh
cargo run -p logisim-rs --release --locked -- examples/agent/counter.circ
cargo run -p logisim-rs --release --locked -- examples/agent/adder.circ
cargo run -p logisim-rs --release --locked -- examples/agent/register-file.circ
```

## Scope

The examples are headless verification artifacts, not screenshots or a fixed
compatibility claim. The register-file example needs the repository's course
plugin at `plugins/course/course.wasm`; if the files are moved, update or
resolve that plugin dependency. External editors may discard unknown
`agent-id` attributes when saving.
