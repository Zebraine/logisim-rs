"""Package a native desktop build and the matching bundled Wasm plugins."""

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import xml.etree.ElementTree as ET
import zipfile


ROOT = Path(__file__).resolve().parents[2]
TARGETS = (
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
)
AGENT_EXAMPLES = ("counter", "adder", "register-file")


def build_matrix(release):
    platforms = [
        ("ubuntu-24.04", TARGETS[0]),
        ("windows-2025", TARGETS[1]),
        ("macos-15", TARGETS[2]),
    ]
    # Intel macOS is retained only for this first release.
    if release == "0.1.0":
        platforms.append(("macos-15-intel", TARGETS[3]))
    return {"include": [{"os": os, "target": target} for os, target in platforms]}


def version():
    with (ROOT / "Cargo.toml").open("rb") as manifest:
        value = tomllib.load(manifest)["workspace"]["package"]["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", value):
        raise ValueError("Release packaging requires a stable semantic version")
    ref = os.environ.get("GITHUB_REF", "")
    if ref.startswith("refs/tags/") and ref != f"refs/tags/v{value}":
        raise ValueError("Release tag does not match the workspace version")
    return value


def check_plugin_paths(dependencies, plugin_paths, circuit):
    for source in dependencies:
        if source["path"] not in plugin_paths or source.get("fallback"):
            raise ValueError(f"Non-portable plugin dependency: {circuit.name}")


def check_native_example(agent, circuit, plugin_paths):
    # Use the same native codec as the desktop before accepting the packaged bytes.
    result = subprocess.run(
        [agent, "lrs", "inspect", "--input", circuit],
        input="", capture_output=True, text=True, encoding="utf-8",
    )
    if result.returncode:
        raise ValueError(f"Cannot open {circuit.name}: {result.stdout or result.stderr}")
    project = json.loads(result.stdout)["data"]
    check_plugin_paths(project["plugin_sources"], plugin_paths, circuit)


def archive_member(info):
    """Store portable metadata, retaining execute permission for the two programs."""
    info.uid = info.gid = info.mtime = 0
    info.uname = info.gname = ""
    program = Path(info.name).name in {"logisim-rs", "logisim-agent"}
    info.mode = 0o755 if info.isdir() or program else 0o644
    return info


def package(target, plugins):
    release = version()
    windows = "windows" in target
    macos = "apple" in target
    binary = "logisim-rs.exe" if windows else "logisim-rs"
    agent_binary = "logisim-agent.exe" if windows else "logisim-agent"
    executable = ROOT / "target" / target / "release" / binary
    agent_executable = ROOT / "target" / target / "release" / agent_binary
    if not executable.is_file():
        raise FileNotFoundError(executable)
    if not agent_executable.is_file():
        raise FileNotFoundError(agent_executable)
    if set(plugins) != {"course", "ttl", "ttl-combinational", "ttl-sequential", "input", "display"}:
        raise ValueError("Provide the course, ttl, ttl-combinational, ttl-sequential, input and display plugins")
    for plugin in plugins.values():
        if not plugin.read_bytes().startswith(b"\0asm\x01\0\0\0"):
            raise ValueError("Expected a built WebAssembly module")
    name = f"logisim-rs-v{release}-{target}"
    destination = ROOT / "dist"
    destination.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="logisim-package-") as temporary:
        staging = Path(temporary) / name
        staging.mkdir()
        for document in ("README.md", "README.zh-CN.md", "LICENSE"):
            shutil.copyfile(ROOT / document, staging / document)
        shutil.copytree(ROOT / "assets", staging / "assets")
        plugin_paths = {f"../../plugins/{name}/{name}.wasm" for name in plugins}
        for group in ("course", "game", "agent"):
            examples = staging / "examples" / group
            examples.mkdir(parents=True)
            source_dir = ROOT / "examples" / group
            if group == "agent":
                circuits = [source_dir / f"{stem}.circ" for stem in AGENT_EXAMPLES]
            else:
                circuits = sorted(source_dir.glob("*.circ"))
            if not circuits:
                raise ValueError(f"No {group} examples found")
            for circuit in circuits:
                tree = ET.parse(circuit)
                sources = tree.find("./lib[@desc='logisim-rs#plugin-v1']/a[@name='sources']")
                if sources is not None:
                    dependencies = json.loads(sources.attrib["val"])
                    if not dependencies:
                        raise ValueError(f"Empty plugin dependency list: {circuit.name}")
                    check_plugin_paths(dependencies, plugin_paths, circuit)
                elif group != "agent":
                    raise ValueError(f"Missing plugin dependency: {circuit.name}")
                shutil.copyfile(circuit, examples / circuit.name)
            if group == "agent":
                for circuit in circuits:
                    native = circuit.with_suffix(".lrs")
                    shutil.copyfile(native, examples / native.name)
                    check_native_example(agent_executable, examples / native.name, plugin_paths)
                shutil.copyfile(source_dir / "README.md", examples / "README.md")
            if group == "game":
                sources = sorted((ROOT / "examples" / group).glob("*.asm"))
                images = sorted((ROOT / "examples" / group).glob("*.hex"))
                if not sources or {s.stem for s in sources} != {s.stem for s in images}:
                    raise ValueError("Game programs require matching .asm and .hex files")
                for program in sources + images:
                    shutil.copyfile(program, examples / program.name)
        legacy = staging / "plugins" / "abi3"
        legacy.mkdir(parents=True)
        for plugin_name, module in plugins.items():
            shutil.copyfile(ROOT / "plugins" / "abi3" / f"{plugin_name}.wasm", legacy / f"{plugin_name}.wasm")
            directory = staging / "plugins" / plugin_name
            directory.mkdir(parents=True)
            shutil.copyfile(module, directory / f"{plugin_name}.wasm")
        if macos:
            contents = staging / "Logisim rs.app" / "Contents"
            (contents / "MacOS").mkdir(parents=True)
            output = contents / "MacOS" / binary
            with (contents / "Info.plist").open("wb") as plist:
                plistlib.dump({
                    "CFBundleExecutable": binary,
                    "CFBundleIdentifier": "org.logisim-rs.desktop",
                    "CFBundleName": "Logisim rs",
                    "CFBundlePackageType": "APPL",
                    "CFBundleShortVersionString": release,
                    "CFBundleVersion": release,
                    "LSMinimumSystemVersion": "15.0",
                    "NSHighResolutionCapable": True,
                }, plist)
        else:
            output = staging / binary
        shutil.copyfile(executable, output)
        output.chmod(0o755)
        agent_output = staging / agent_binary
        shutil.copyfile(agent_executable, agent_output)
        agent_output.chmod(0o755)
        if windows:
            archive = destination / f"{name}.zip"
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as bundle:
                for file in sorted(staging.rglob("*")):
                    if file.is_file():
                        bundle.write(file, file.relative_to(staging.parent))
        else:
            archive = destination / f"{name}.tar.gz"
            with archive.open("wb") as raw, gzip.GzipFile(
                filename="", mode="wb", fileobj=raw, compresslevel=9, mtime=0
            ) as compressed, tarfile.open(fileobj=compressed, mode="w") as bundle:
                bundle.add(staging, arcname=name, filter=archive_member)
        with archive.open("rb") as built:
            digest = hashlib.file_digest(built, "sha256").hexdigest()
        archive.with_name(archive.name + ".sha256").write_text(
            f"{digest}  {archive.name}\n", encoding="utf-8"
        )
        print(archive)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target", nargs="?", choices=TARGETS)
    parser.add_argument("--plugin", action="append", metavar="NAME=PATH")
    parser.add_argument("--version", action="store_true")
    parser.add_argument("--matrix", action="store_true")
    args = parser.parse_args()
    if args.version:
        print(version())
    elif args.matrix:
        matrix = build_matrix(version())
        print("matrix=" + json.dumps(matrix, separators=(",", ":")))
        print("package_count=" + str(len(matrix["include"])))
    elif args.target and args.plugin:
        plugins = {}
        for item in args.plugin:
            name, separator, path = item.partition("=")
            if not separator or not path or name in plugins:
                parser.error("Use distinct --plugin NAME=PATH entries")
            plugins[name] = Path(path)
        package(args.target, plugins)
    else:
        parser.error("Provide a build target and --plugin, or use --version")
