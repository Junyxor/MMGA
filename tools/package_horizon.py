"""Package MMGA as a source-only native mod for the pinned Horizon SDK.

The checkout keeps its three Cargo crates. Only the archive gets standalone
manifests and local dependencies relocated below src/deps, as Horizon requires.
No SDK, third-party Java jars, compiler outputs or downloaded source is shipped.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import tomllib
import zipfile
from io import BytesIO


ROOT = Path(__file__).resolve().parent.parent
CRATES = ("mmga-classfile", "mmga-runtime", "mmga-neoforge")
SDK_PATH = ".horizon/sdk/api/crates/horizon-mod-api"
PIN = "0b14f8acfd97fa2c750e42795249dcc981178ca4"
MIXINS = "e59da9094f0c2fdda6c627fe9eff22dec6a906f0"


def scalar(value: object) -> str:
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, bool):
        return str(value).lower()
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, list):
        return "[" + ", ".join(scalar(item) for item in value) + "]"
    if isinstance(value, dict):
        return "{ " + ", ".join(f"{json.dumps(key)} = {scalar(item)}" for key, item in value.items()) + " }"
    raise ValueError(f"unsupported manifest value {value!r}")


def cargo_toml(manifest: dict) -> bytes:
    """A deterministic TOML writer for Cargo's tables and array-of-table targets."""
    lines: list[str] = ["# Generated for the Horizon package; edit the original checkout manifests."]

    def table(values: dict, path: tuple[str, ...], array: bool = False) -> None:
        if path:
            name = ".".join(json.dumps(part) for part in path)
            lines.extend(["", f"[[{name}]]" if array else f"[{name}]"])
        for key, value in values.items():
            if not isinstance(value, dict) and not (isinstance(value, list) and value and isinstance(value[0], dict)):
                lines.append(f"{json.dumps(key)} = {scalar(value)}")
        for key, value in values.items():
            if isinstance(value, dict):
                table(value, path + (key,))
            elif isinstance(value, list) and value and isinstance(value[0], dict):
                for item in value:
                    table(item, path + (key,), True)

    table(manifest, ())
    result = ("\n".join(lines) + "\n").encode("utf-8")
    if tomllib.loads(result.decode()) != manifest:
        raise ValueError("generated Cargo manifest did not round-trip")
    return result


def source_bytes(path: Path) -> bytes:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"source is not a regular file: {path}")
    # Windows junctions are not portable source paths either.
    if os.name == "nt" and getattr(path.stat(follow_symlinks=False), "st_file_attributes", 0) & 0x400:
        raise ValueError(f"linked source is refused: {path}")
    return path.read_bytes()


def sources(directory: Path) -> list[Path]:
    if not directory.is_dir() or directory.is_symlink():
        raise ValueError(f"missing or linked source directory: {directory}")
    result = []
    for current, dirs, files in os.walk(directory, followlinks=False):
        dirs[:] = [name for name in dirs if name not in ("tests", "test", "fixtures") and not name.startswith("test_")]
        for name in dirs:
            candidate = Path(current, name)
            if candidate.is_symlink() or (os.name == "nt" and candidate.stat().st_file_attributes & 0x400):
                raise ValueError(f"linked source directory is refused: {candidate}")
        for name in files:
            path = Path(current, name)
            if name in ("tests.rs", "test.rs", "fixture.rs") or name.startswith("test_") or name.endswith("_tests.rs"):
                continue
            if path.suffix in (".rs", ".toml"):
                result.append(path)
    return sorted(result)


def standalone(original: dict, workspace: dict, owner: str, root: bool = False) -> dict:
    manifest = copy.deepcopy(original)
    for refused in ("patch", "replace", "features"):
        if refused in manifest:
            raise ValueError(f"{owner}: [{refused}] is unsupported by the pinned native build")
    manifest.pop("workspace", None)  # Local checkout isolation is not part of a native package.
    package = manifest.get("package", {})
    if package.get("build") not in (None, False):
        raise ValueError(f"{owner}: native mod build scripts are refused")
    for key, value in list(package.items()):
        if isinstance(value, dict) and value.get("workspace"):
            if key not in workspace.get("package", {}):
                raise ValueError(f"{owner}: missing workspace package value {key}")
            package[key] = copy.deepcopy(workspace["package"][key])
    package.update(autobins=False, autoexamples=False, autotests=False, autobenches=False)
    for target in ("bin", "example", "test", "bench", "dev-dependencies", "dev_dependencies"):
        manifest.pop(target, None)
    if manifest.get("lints", {}).get("workspace"):
        manifest["lints"] = copy.deepcopy(workspace.get("lints", {}))

    def dependencies(table: dict) -> None:
        for section in ("dependencies", "build-dependencies", "build_dependencies"):
            for name, value in table.get(section, {}).items():
                if isinstance(value, dict) and value.get("workspace"):
                    inherited = workspace.get("dependencies", {}).get(name)
                    if inherited is None:
                        raise ValueError(f"{owner}: missing workspace dependency {name}")
                    inherited = {"version": inherited} if isinstance(inherited, str) else copy.deepcopy(inherited)
                    inherited.update({key: item for key, item in value.items() if key != "workspace"})
                    value = table[section][name] = inherited
                if not isinstance(value, dict):
                    continue
                if any(key in value for key in ("git", "rev", "tag", "branch", "registry", "registry-index")):
                    raise ValueError(f"{owner}: non-crates.io dependency {name} is refused")
                if name in CRATES:
                    value["path"] = f"src/deps/{name}" if root else f"../{name}"
                elif name == "horizon-mod-api":
                    if not root:
                        raise ValueError(f"{owner}: only the adapter may depend on the Horizon SDK")
                    value["path"] = SDK_PATH
                    # The normal mod dependency uses only the pinned SDK defaults.
                    # Dev-dependencies are removed from the package and may enable
                    # the SDK's official testing fixtures in the checkout.
                    if section == "dependencies" and ("features" in value or value.get("optional")):
                        raise ValueError("native mods cannot request custom SDK features")
                elif "path" in value:
                    raise ValueError(f"{owner}: undeclared local dependency {name} is refused")

    dependencies(manifest)
    for target in manifest.get("target", {}).values():
        dependencies(target)
    return manifest


def collect(root: Path = ROOT) -> dict[str, bytes]:
    workspace = tomllib.loads(source_bytes(root / "Cargo.toml").decode())["workspace"]
    adapter = root / "horizon"
    text = source_bytes(adapter / "horizon-mod.toml")
    manifest = tomllib.loads(text.decode())
    if manifest.get("native") != {"game": "26.3", "horizon": PIN, "mixins": MIXINS}:
        raise ValueError("Horizon build pins differ from the reviewed SDK")
    lockfile = adapter / "Cargo.lock"
    if not lockfile.is_file():
        raise ValueError("native mod Cargo.lock is missing; run the pinned native-lock command first")
    files = {"horizon-mod.toml": text, "Cargo.lock": source_bytes(lockfile)}
    root_manifest = tomllib.loads(source_bytes(adapter / "Cargo.toml").decode())
    files["Cargo.toml"] = cargo_toml(standalone(root_manifest, workspace, "mmga-horizon", True))
    for file in sources(adapter / "src"):
        files["src/" + file.relative_to(adapter / "src").as_posix()] = source_bytes(file)
    for crate in CRATES:
        directory = root / "crates" / crate
        prefix = f"src/deps/{crate}/"
        cargo = tomllib.loads(source_bytes(directory / "Cargo.toml").decode())
        files[prefix + "Cargo.toml"] = cargo_toml(standalone(cargo, workspace, crate))
        for file in sources(directory / "src"):
            files[prefix + "src/" + file.relative_to(directory / "src").as_posix()] = source_bytes(file)
    for tree in ("assets", "data"):
        directory = adapter / tree
        if directory.exists():
            for current, dirs, names in os.walk(directory, followlinks=False):
                for name in dirs:
                    if Path(current, name).is_symlink():
                        raise ValueError(f"linked content directory is refused: {name}")
                for name in names:
                    file = Path(current, name)
                    files[file.relative_to(adapter).as_posix()] = source_bytes(file)
    for name in ("README.md", "LICENSE", "NOTICE.md"):
        if (adapter / name).exists():
            files[name] = source_bytes(adapter / name)
    validate(files)
    return files


def validate(files: dict[str, bytes]) -> None:
    for name, data in files.items():
        path = PurePosixPath(name)
        if not name or "\\" in name or ":" in name or name.startswith("/") or any(part in ("", ".", "..") for part in name.split("/")):
            raise ValueError(f"invalid archive path: {name}")
        allowed = name in ("horizon-mod.toml", "Cargo.toml", "Cargo.lock", "README.md", "LICENSE", "NOTICE.md")
        allowed |= name.startswith("src/") and path.suffix in (".rs", ".toml")
        allowed |= name.startswith(("assets/", "data/"))
        if not allowed or path.suffix.lower() in (".jar", ".class", ".exe", ".dll", ".so", ".dylib", ".wasm", ".ps1", ".bat", ".cmd"):
            raise ValueError(f"unsupported package file: {name}")
        if data.startswith((b"MZ", b"\x7fELF", b"\0asm", b"\xca\xfe\xba\xbe")) or (data.startswith(b"#!") and not data.startswith(b"#![")):
            raise ValueError(f"executable payload is refused: {name}")
    if "src/lib.rs" not in files:
        raise ValueError("missing native adapter src/lib.rs")
    selected = [data for name, data in files.items() if name != "horizon-mod.toml" and not name.startswith(("assets/", "data/"))]
    if len(selected) > 511 or sum(map(len, selected)) > 64 << 20:
        raise ValueError("native package exceeds 511 files or 64 MiB of source")
    lock = tomllib.loads(files["Cargo.lock"].decode())
    pins = {entry["name"] for entry in lock.get("package", []) if entry.get("source") == "registry+https://github.com/rust-lang/crates.io-index" and len(entry.get("checksum", "")) == 64}
    for name, data in files.items():
        if not name.endswith("Cargo.toml"):
            continue
        for section in ("dependencies", "build-dependencies"):
            for dependency, value in tomllib.loads(data.decode()).get(section, {}).items():
                if isinstance(value, dict) and "path" in value:
                    continue
                package = value.get("package", dependency) if isinstance(value, dict) else dependency
                if package not in pins:
                    raise ValueError(f"Cargo.lock lacks a checksummed registry pin for {package}")


def archive(files: dict[str, bytes]) -> bytes:
    result = BytesIO()
    with zipfile.ZipFile(result, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as package:
        for name, data in sorted(files.items()):
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = (stat.S_IFREG | 0o644) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            package.writestr(info, data, compresslevel=9)
    return result.getvalue()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/horizon-mod/mmga.hmod")
    parser.add_argument("--stage-dir", type=Path, help="also write the identical source tree for official validation")
    args = parser.parse_args()
    try:
        files = collect()
        data = archive(files)
        if args.stage_dir:
            if args.stage_dir.exists() and any(args.stage_dir.iterdir()):
                raise ValueError("stage directory must be empty; existing files are never deleted")
            for name, content in files.items():
                destination = args.stage_dir / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(content)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        partial = args.output.with_suffix(".hmod.partial")
        partial.write_bytes(data)
        partial.replace(args.output)
        print(json.dumps({"package": str(args.output.resolve()), "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data), "files": len(files), "horizon": PIN}, indent=2))
        return 0
    except (OSError, ValueError, KeyError, tomllib.TOMLDecodeError) as error:
        print(f"package_horizon: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
