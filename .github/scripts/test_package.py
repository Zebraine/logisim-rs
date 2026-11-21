"""Exercise the shipped Agent and showcase from a freshly extracted native archive."""

import argparse
import hashlib
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import tarfile
import tempfile


def check_archive_metadata(archive):
    with archive.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
        stream.seek(0)
        if archive.name.endswith(".tar.gz") and stream.read(8)[3:8] != bytes(5):
            raise ValueError("Unexpected gzip flags or timestamp")
    if Path(str(archive) + ".sha256").read_text().split() != [digest, archive.name]:
        raise ValueError(f"Checksum mismatch for {archive.name}")
    if archive.name.endswith(".tar.gz"):
        with tarfile.open(archive, "r:gz") as bundle:
            for entry in bundle:
                program = Path(entry.name).name in {"logisim-rs", "logisim-agent"}
                mode = 0o755 if entry.isdir() or program else 0o644
                if (entry.uid, entry.gid, entry.mtime, entry.uname, entry.gname, entry.mode) != (
                    0, 0, 0, "", "", mode
                ) or not (entry.isfile() or entry.isdir()):
                    raise ValueError(f"Unexpected archive metadata for {entry.name}")


def check_inventory(root, package, names):
    expected = {Path(p) for p in ("README.md", "README.zh-CN.md", "LICENSE", "examples/agent/README.md")}
    expected.update(p.relative_to(root) for p in (root / "assets").glob("*.png"))
    for group, suffixes in [("course", {".circ"}), ("game", {".circ", ".asm", ".hex"})]:
        expected.update(p.relative_to(root) for p in (root / "examples" / group).iterdir()
                        if p.suffix in suffixes)
    expected.update(Path("examples/agent") / f"{name}.{ext}" for name in names for ext in ("lrs", "circ"))
    for rel in expected:
        if (package / rel).read_bytes() != (root / rel).read_bytes():
            raise ValueError(f"Packaged file differs from its source: {rel}")
    for plugin in ("course", "ttl", "ttl-combinational", "ttl-sequential", "input", "display"):
        expected.add(Path("plugins") / plugin / f"{plugin}.wasm")
    if package.name.endswith("-apple-darwin"):
        contents = Path("Logisim rs.app/Contents")
        expected.update({contents / "MacOS/logisim-rs", contents / "Info.plist", Path("logisim-agent")})
    else:
        suffix = ".exe" if os.name == "nt" else ""
        expected.update(Path(program + suffix) for program in ("logisim-rs", "logisim-agent"))
    directories = {parent for rel in expected for parent in rel.parents if parent != Path(".")}
    actual = {p.relative_to(package) for p in package.rglob("*")}
    if actual != expected | directories:
        raise ValueError(f"Unexpected or missing package paths: {sorted(actual ^ (expected | directories))}")
    if not all((package / rel).is_file() for rel in expected):
        raise ValueError("Expected regular files in the package")


def check_archive(archive):
    root = Path(__file__).resolve().parents[2]
    # Use the same probe/input trajectories as the generator, with the Agent in the archive.
    showcase = runpy.run_path(str(root / "examples/agent/build_showcase.py"))
    names = showcase["NAMES"]
    session, call, verify = showcase["session"], showcase["call"], showcase["verify"]
    check_archive_metadata(archive)
    with tempfile.TemporaryDirectory(prefix="logisim-package-test-") as tmp:
        shutil.unpack_archive(archive, tmp)
        package, = Path(tmp).iterdir()
        check_inventory(root, package, names)
        examples = package / "examples/agent"

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
