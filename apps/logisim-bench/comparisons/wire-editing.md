# Desktop wire editing at high clock rates

Linux x86-64, i5-12600KF, Rust 1.99.0, Xvfb at 1200×820 and 96 DPI, release desktop with default features. The comparison uses the same location, clock stimulus, pointer sequence and 100% canvas zoom on the Stage 14 baseline `c8a1912` and the Stage 15 revision containing this report. Each rate starts a fresh process with its own preferences and project copy.

## Reproduction

Generate the fixture and open it in the desktop app.

```sh
python3 apps/logisim-bench/comparisons/wire-editing.py > /tmp/wire-editing.circ
target/release/logisim-rs /tmp/wire-editing.circ
```

1. Use actual size (100%), Stable mode, and automatic ticks at 1000, 8192 or 32768 Hz. The clock drives one input of a NOR. The other input has an open return path with eleven right-angle bends in each direction. There are 4000 disconnected background wires and one loose foreground wire.
2. Measure the open circuit at rest and while moving the pointer. Ctrl-click the loose wire around world (120,270). Drag its end from (150,270) to (300,270), moving across several grid rows, then release.
3. With the Wire tool, drag from (190,40) to (300,40). Leave the completed segment selected. A clock edge clamps the gate to a known value. Removing the clamp starts oscillation. Move the pointer without further edits for seven seconds, then leave the window untouched for 2.5 seconds.
4. Undo or delete the closing wire and check the actual tick counter. The worker should settle and resume automatic edges. The generator also accepts `--closed` for opening the finished loop directly.

The XTest replay sent pointer motions at 60 Hz. Frame intervals below are delivery times of XDamage notifications for the application window. Input latency pairs each pointer coordinate consumed by egui with the first XDamage update after that UI pass. This includes desktop processing and software rendering up to the X server. Physical display scanout was outside the measurement. The worker measurement used thread CPU time from Linux schedstat. Temporary probes counted configurations, propagation batches, snapshot entries and producer-side inbox lock waits. They are absent from the release build.

## Paired window measurements

Frame times are p50 / p95 / maximum, in milliseconds. Closed-loop FPS uses the seven-second interval with pointer input and no edits.

| Hz | Baseline drag (ms) | Fixed drag (ms) | Baseline closed-loop FPS | Fixed closed-loop FPS | Fixed closed-loop intervals (ms) | Fixed drag input p95 (ms) |
| ---: | --- | --- | ---: | ---: | --- | ---: |
| 1000 | 27.01 / 142.85 / 154.41 | 7.84 / 10.89 / 18.57 | 0.57 | 128.28 | 7.64 / 9.04 / 15.42 | 17.93 |
| 8192 | 25.12 / 67.84 / 84.15 | 7.86 / 11.00 / 16.28 | 0.57 | 124.28 | 7.71 / 9.50 / 18.48 | 17.68 |
| 32768 | 24.89 / 68.67 / 83.05 | 7.82 / 10.46 / 13.10 | 0.57 | 126.14 | 7.68 / 8.99 / 14.72 | 17.03 |

On the baseline, a **single selected-wire connectivity query** on the closed net took about 1.74 s at 32768 Hz. The inspector ran it on each redraw and the canvas also needed it on selection changes. One endpoint drag sent 18 electrical configurations to the worker at that rate. On the candidate, the inspector and drawing share the indexed net selection, preview frames send zero configurations, and release sends one. Adding the closing wire sends one more.

| Hz | Acyclic pointer control p95 before / after (ms) | Worker CPU during drag before / after (%) | Fault snapshot after release (ms) | X11 updates in first second after release | First advancing snapshot after undo (ms) |
| ---: | --- | --- | ---: | ---: | ---: |
| 1000 | 29.18 / 10.29 | 7.95 / 4.54 | 11.8 | 111 | 69.3 |
| 8192 | 11.61 / 8.74 | 7.78 / 5.98 | 27.6 | 109 | 56.4 |
| 32768 | 11.89 / 9.89 | 10.25 / 8.26 | 25.6 | 101 | 41.0 |

At 32768 Hz the open circuit published 84 snapshots in 1.4 s. The approximately 2.25 s drag interval including its trailing hold accumulated 73,251 propagation batches in 50.10 ms and 134 snapshots. Each snapshot held at most 4032 wire/port entries (about 63 KiB of map keys and shared pointers alone). Peak queued commands during the drag was 1, and the longest measured producer lock acquisition was 240 ns. The single-slot mailbox retains the latest presentation.

After the final fault snapshot, the candidate produced **zero** additional propagation batches, ticks or snapshots during the seven seconds of input and the following idle period. Worker CPU was 0.00% in both intervals at every rate. The terminal snapshot retained the two existing propagation-limit diagnostics and the pending drive. An untouched window then produced 4–5 telemetry redraws per second. Input-driven rendering recovered within the first second without a continuous repaint request. Before the fix, undo cleared the limit but left the worker disabled. After the fix, the first accepted edges after undo appeared within 70 ms at all rates.

A separate uninstrumented release run at 32768 Hz used Delete on the selected closing wire. Drag p95 was 10.98 ms. Closed-loop interaction ran at 128.56 window updates/s with the worker at 0.00% CPU. After Delete the worker resumed (9.25% CPU) and interaction ran at 121.66 updates/s. A repeat with counters saw the first advancing snapshot 24.4 ms after Delete, and 97,936 additional edges between the first and last snapshots of the three-second recovery interval.

## Regression checks

The workspace exercises manual event steps, stable sampling, plugin faults and high-rate commands. A deterministic feedback test checks that a terminal limit stays parked across polls and language changes, and that fixing the topology preserves an explicitly paused simulation. A threaded test submits 32 electrical edits while clocks run, closes and reopens the loop and checks port values and accepted tick accounting. The UI test drives move and endpoint gestures through release, Escape and undo.

The selected-wire CPU comparison can also run without X11. It prints egui frame-build percentiles for open and closed wire geometry and verifies the net membership and unchanged document after forty frames in each case.

```sh
cargo test --release -p logisim-ui selected_wire_frames_with_an_acyclic_control -- --nocapture
```

The first implementation shared the connectivity pass with the simulation compiler. Eight interleaved release process pairs on CPU 15 reproduced a deep-chain throughput loss (12,377 to 11,488 iterations/s, median). Six more pairs showed that inlining did not recover it. The final indexed selection pass uses the core wire index only in the UI. The model and simulation crates are byte-for-byte unchanged from Stage 14. The before/after release `logisim-bench --no-default-features` executables have the same SHA-256, `92b0321b9ed78e3f5d4ece93d932af573bd1e8b29d05c90753d71cc1e7375df4`.

| Headless control | Four-state digest | Ticks | Steps | Diagnostics |
| --- | --- | ---: | ---: | ---: |
| Wide | `a817e833dd81da75` | 0 | 512 | 0 |
| Deep | `17090361c775bb25` | 0 | 512 | 0 |
| Single-cycle CPU | `73171884c0465aad` | 128 | 771 | 0 |
| Register-file plugin | `a2689cffb70ff825` | 512 | 1536 | 0 |
