"""Linux release MCP stdio benchmark. Run from any cwd; emits one JSON report."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import statistics
import subprocess
import tempfile
import time
from contextlib import contextmanager

REPO = Path(__file__).resolve().parents[3]
PROJECT = Path("examples/course/single-cycle.circ")
PLUGIN = Path("plugins/course/course.wasm")
# Read the correlation ID without decoding two full states inside the timing loop.
# A different object field order takes the general JSON path.
FIRST_ID = re.compile(rb'^\{"id":([0-9]+),')
TIMEOUT = 30


def ms(start):
    return (time.perf_counter_ns() - start) / 1e6


def quantiles(samples):
    return {"p50": statistics.median(samples),
            "p95": sorted(samples)[math.ceil(len(samples) * .95) - 1]}


def send(server, message):
    server.stdin.write(json.dumps(message, separators=(",", ":")).encode() + b"\n")
    server.stdin.flush()


def read(server):
    signal.alarm(TIMEOUT)
    try:
        raw = server.stdout.readline()
    finally:
        signal.alarm(0)
    if not raw.endswith(b"\n"):
        raise RuntimeError("agent closed stdout mid-request")
    return raw


@contextmanager
def agent(args, root, mode):
    command = [args.agent, "--root", root, mode]
    if args.agent_cpus:
        command = ["taskset", "-c", args.agent_cpus, *command]
    with subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE) as server:
        try:
            yield server
            server.stdin.close()
            if server.wait(timeout=TIMEOUT):
                raise RuntimeError("agent exited unsuccessfully")
        finally:
            if server.poll() is None:
                server.kill()
                server.wait()


def rpc(server, request_id, method, params):
    send(server, {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})


def tool(server, request_id, name, arguments):
    rpc(server, request_id, "tools/call", {"name": name, "arguments": arguments})


def data(envelope):
    if not envelope["ok"]:
        raise RuntimeError(envelope["error"])
    return envelope["data"]


def checked_reply(raw, expected_id):
    reply = json.loads(raw)
    if reply["id"] != expected_id or reply["jsonrpc"] != "2.0":
        raise RuntimeError("misrouted JSON-RPC response")
    result = reply["result"]
    envelope = result["structuredContent"]
    if result["isError"] or json.loads(result["content"][0]["text"]) != envelope:
        raise RuntimeError(f"MCP result mismatch or tool failure for {expected_id}")
    return data(envelope)


def names(index):
    return "simulation_state" if index % 2 else "simulation_tick"


def update_digest(digest, result, index, session, root):
    tick = index // 2 + 1
    if result.pop("session_id") != session:
        raise RuntimeError("session mismatch")
    if index % 2:
        state = result
    else:
        state = result["state"]
        if state.pop("session_id") != session:
            raise RuntimeError("nested state belongs to a different session")
    if (state["ticks"] != tick or state["engine"] != "stable"
            or state["wasm_engine"] != "wasmi" or state["error"] or state["diagnostics"]):
        raise RuntimeError(f"unexpected simulation state at tick {tick}")
    for plugin in state["plugins"]:
        if plugin["status"] != "loaded":
            raise RuntimeError("course plugin missing")
        plugin["path"] = str(Path(plugin["path"]).resolve().relative_to(root))
    digest.update(json.dumps(result, sort_keys=True, separators=(",", ":")).encode() + b"\n")


def reference(args, root):
    with agent(args, root, "cli") as server:
        def call(name, arguments):
            send(server, {"tool": name, "arguments": arguments})
            return data(json.loads(read(server)))

        session = call("project_open", {"path": str(PROJECT)})["session_id"]
        arguments = {"session": session}
        call("simulation_open", arguments)
        digest = hashlib.sha256()
        for index in range(args.pairs * 2):
            update_digest(digest, call(names(index), arguments), index, session, root)
        return digest.hexdigest()


def scheduling(threads):
    # Linux schedstat reports CPU service time and runnable wait time in ns.
    return [list(map(int, (thread / "schedstat").read_text().split()[:2]))
            for thread in threads]


def run(args, root, count, expected):
    with agent(args, root, "mcp") as server:
        rpc(server, 0, "initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                      "clientInfo": {"name": "bench", "version": "1"}})
        if json.loads(read(server))["result"]["protocolVersion"] != "2025-11-25":
            raise RuntimeError("MCP handshake failed")
        send(server, {"jsonrpc": "2.0", "method": "notifications/initialized"})
        sessions, opens, loads = [], [], []
        workers = []
        threads = Path(f"/proc/{server.pid}/task")
        for lane in range(count):
            before = set(threads.iterdir())

            def cold(name, arguments, timings):
                start = time.perf_counter_ns()
                tool(server, 0, name, arguments)
                raw = read(server)
                timings.append(ms(start))
                return checked_reply(raw, 0)

            session = cold("project_open", {"path": str(PROJECT.with_name(f"cpu-{lane}.circ"))},
                           opens)["session_id"]
            worker, = set(threads.iterdir()) - before
            workers.append(worker)
            if args.worker_cpus:
                os.sched_setaffinity(int(worker.name), {args.worker_cpus[lane]})
            state = cold("simulation_open", {"session": session}, loads)
            if not state["plugins"] or state["ticks"] != 0:
                raise RuntimeError("invalid initial state")
            sessions.append(session)

        length = args.pairs * 2
        pending, captured = {}, []
        latency = [[] for _ in sessions]

        def submit(lane, index):
            request_id = 1 + lane * length + index
            pending[request_id] = (lane, index, time.perf_counter_ns())
            tool(server, request_id, names(index), {"session": sessions[lane]})

        scheduler_before = scheduling(workers)
        start = time.perf_counter_ns()
        for lane in range(count):
            submit(lane, 0)
        while pending:
            raw = read(server)
            completed = time.perf_counter_ns()
            match = FIRST_ID.match(raw)
            request_id = int(match[1]) if match else json.loads(raw)["id"]
            lane, index, submitted = pending.pop(request_id)
            latency[lane].append((completed - submitted) / 1e6)
            captured.append((raw, request_id, lane, index))
            if index + 1 < length:
                submit(lane, index + 1)
        seconds = (completed - start) / 1e9
        scheduler_ms = [[(a - b) / 1e6 for a, b in zip(after, before)]
                        for after, before in zip(scheduling(workers), scheduler_before)]
        status = Path(f"/proc/{server.pid}/status").read_text()
        rss = int(re.search(r"^VmHWM:\s+(\d+) kB$", status, re.M)[1])
        hashes = [hashlib.sha256() for _ in sessions]
        for raw, request_id, lane, index in captured:
            result = checked_reply(raw, request_id)
            update_digest(hashes[lane], result, index, sessions[lane], root)
        digests = [digest.hexdigest() for digest in hashes]
        if any(digest != expected for digest in digests):
            raise RuntimeError("concurrent trajectory diverged from serial CLI")
        for session in sessions:
            tool(server, 0, "project_close", {"session": session})
            checked_reply(read(server), 0)
        return {"sessions": count, "requests_per_s": count * length / seconds,
                "pairs_per_s": count * args.pairs / seconds, "peak_rss_kib": rss,
                "wall_ms": seconds * 1000, "worker_cpu_ms": [cpu for cpu, _ in scheduler_ms],
                "worker_runnable_wait_ms": [wait for _, wait in scheduler_ms],
                "request_ms": [quantiles(row) for row in latency],
                "state_ms": [quantiles(row[1::2]) for row in latency],
                "project_open_ms": opens, "simulation_open_including_plugins_ms": loads,
                "digests": digests}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agent", type=Path, default=REPO / "target/release/logisim-agent")
    parser.add_argument("--agent-cpus", help="Linux taskset CPU list for server only")
    parser.add_argument("--worker-cpus", type=lambda s: [int(cpu) for cpu in s.split(",")],
                        help="pin the four session workers to distinct Linux CPUs, in lane order")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--pairs", type=int, default=32)
    args = parser.parse_args()
    if args.worker_cpus and (len(args.worker_cpus) != 4 or len(set(args.worker_cpus)) != 4):
        parser.error("worker-cpus must contain four distinct CPU indices")
    if min(args.runs, args.pairs) < 1:
        parser.error("runs and pairs must be positive")

    def timeout(*_):
        raise TimeoutError(f"no complete response within {TIMEOUT}s")

    signal.signal(signal.SIGALRM, timeout)
    with tempfile.TemporaryDirectory(prefix="logisim-mcp-bench-") as directory:
        root = Path(directory).resolve()
        for file in (PROJECT, PLUGIN):
            (root / file).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REPO / file, root / file)
        for lane in range(4):
            shutil.copyfile(root / PROJECT, root / PROJECT.with_name(f"cpu-{lane}.circ"))
        digest = reference(args, root)
        rows = []
        failures = []
        groups = (1, 2, 4)
        for repeat in range(args.runs):
            # Rotate process start order across repeats.
            for offset in range(len(groups)):
                count = groups[(repeat + offset) % len(groups)]
                try:
                    rows.append({"repeat": repeat + 1, **run(args, root, count, digest)})
                except (RuntimeError, TimeoutError, subprocess.TimeoutExpired, OSError) as error:
                    failures.append({"repeat": repeat + 1, "sessions": count,
                                     "type": type(error).__name__, "message": str(error)})
        if failures:
            print(json.dumps({"cli_digest": digest, "trials": rows, "failures": failures}, indent=2))
            raise SystemExit(1)
        summary = {}
        for count in groups:
            trials = [row for row in rows if row["sessions"] == count]
            rate = statistics.median(row["pairs_per_s"] for row in trials)
            tail = statistics.median(max(lane["p95"] for lane in row["request_ms"]) for row in trials)
            summary[count] = {"pairs_per_s": rate, "requests_per_s": 2 * rate,
                              "worst_lane_p95_ms": tail,
                              "speedup": rate / summary[1]["pairs_per_s"] if count > 1 else 1,
                              "p95_ratio": tail / summary[1]["worst_lane_p95_ms"] if count > 1 else 1}
        print(json.dumps({"pairs_per_session": args.pairs, "agent_cpus": args.agent_cpus,
                          "worker_cpus": args.worker_cpus, "client_cpus": sorted(os.sched_getaffinity(0)), "cli_digest": digest,
                          "summary": summary, "trials": rows, "failures": failures}, indent=2))


if __name__ == "__main__":
    main()
