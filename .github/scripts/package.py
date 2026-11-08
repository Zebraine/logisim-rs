"""Package a native desktop build and the matching shared Wasm plugin."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
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


def version():
    with (ROOT / "Cargo.toml").open("rb") as manifest:
        value = tomllib.load(manifest)["workspace"]["package"]["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", value):
        raise ValueError("Release packaging requires a stable semantic version")
    ref = os.environ.get("GITHUB_REF", "")
    if ref.startswith("refs/tags/") and ref != f"refs/tags/v{value}":
        raise ValueError("Release tag does not match the workspace version")
    return value


def package(target, plugin):
    release = version()
    windows = "windows" in target
    macos = "apple" in target
    binary = "logisim-rs.exe" if windows else "logisim-rs"
    executable = ROOT / "target" / target / "release" / binary
    if not executable.is_file():
        raise FileNotFoundError(executable)
    if not plugin.read_bytes().startswith(b"\0asm\x01\0\0\0"):
        raise ValueError("Expected a built WebAssembly module")
    name = f"logisim-rs-v{release}-{target}"
    destination = ROOT / "dist"
    destination.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="logisim-package-") as temporary:
        staging = Path(temporary) / name
        staging.mkdir()
        for document in ("README.md", "LICENSE"):
            shutil.copyfile(ROOT / document, staging / document)
        examples = staging / "examples" / "course"
        examples.mkdir(parents=True)
        circuits = sorted((ROOT / "examples" / "course").glob("*.circ"))
        if not circuits:
            raise ValueError("No course examples found")
        for circuit in circuits:
            tree = ET.parse(circuit)
            sources = tree.find("./lib[@desc='logisim-rs#plugin-v1']/a[@name='sources']")
            if sources is None:
                raise ValueError(f"Missing plugin dependency: {circuit.name}")
            dependencies = json.loads(sources.attrib["val"])
            if len(dependencies) != 1 or any(
                s["path"] != "course.wasm" or s.get("fallback")
                for s in dependencies
            ):
                raise ValueError(f"Non-portable plugin dependency: {circuit.name}")
            shutil.copyfile(circuit, examples / circuit.name)
        shutil.copyfile(plugin, examples / "course.wasm")
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
        if windows:
            archive = destination / f"{name}.zip"
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as bundle:
                for file in sorted(staging.rglob("*")):
                    if file.is_file():
                        bundle.write(file, file.relative_to(staging.parent))
        else:
            archive = destination / f"{name}.tar.gz"
            with tarfile.open(archive, "w:gz") as bundle:
                bundle.add(staging, arcname=name)
        with archive.open("rb") as built:
            digest = hashlib.file_digest(built, "sha256").hexdigest()
        archive.with_name(archive.name + ".sha256").write_text(
            f"{digest}  {archive.name}\n", encoding="utf-8"
        )
        print(archive)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target", nargs="?", choices=TARGETS)
    parser.add_argument("--plugin", type=Path)
    parser.add_argument("--version", action="store_true")
    args = parser.parse_args()
    if args.version:
        print(version())
    elif args.target and args.plugin:
        package(args.target, args.plugin)
    else:
        parser.error("Provide a build target and --plugin, or use --version")
