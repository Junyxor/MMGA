"""Observable archive contract tests; no game or SDK download is required."""

from io import BytesIO
from pathlib import Path
import tempfile
import tomllib
import unittest
import zipfile

import package_horizon as package


class NativePackageTests(unittest.TestCase):
    def test_collect_packages_three_original_crates_and_registry_lock(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "Cargo.toml").write_text('''[workspace]
[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
[workspace.dependencies]
serde = "1.0"
''', encoding="utf-8")
            lock = 'version = 4\n[[package]]\nname="serde"\nversion="1.0.0"\nsource="registry+https://github.com/rust-lang/crates.io-index"\nchecksum="' + "0" * 64 + '"\n'
            (root / "Cargo.lock").write_text(lock, encoding="utf-8")
            native = root / "horizon"
            (native / "src").mkdir(parents=True)
            (native / "horizon-mod.toml").write_bytes((package.ROOT / "horizon/horizon-mod.toml").read_bytes())
            (native / "Cargo.lock").write_text(lock, encoding="utf-8")
            (native / "src/lib.rs").write_text("pub fn adapter() {}\n", encoding="utf-8")
            (native / "src/tests.rs").write_text('const STUB: &str = "org/neoforged/Test";\n', encoding="utf-8")
            (native / "src/bridge_tests.rs").write_text("// Contract tests only\n", encoding="utf-8")
            (native / "Cargo.toml").write_text('''[package]
name="mmga-horizon"
version="0.1.0"
[workspace]
[dependencies]
horizon-mod-api={path="../sdk"}
mmga-neoforge={path="../crates/mmga-neoforge"}
''', encoding="utf-8")
            for crate in package.CRATES:
                directory = root / "crates" / crate
                (directory / "src").mkdir(parents=True)
                (directory / "src/lib.rs").write_text(f"// {crate} original source\n", encoding="utf-8")
                (directory / "Cargo.toml").write_text(f'''[package]
name="{crate}"
version.workspace=true
edition.workspace=true
[dependencies]
serde.workspace=true
''', encoding="utf-8")
            files = package.collect(root)
            self.assertNotIn("src/tests.rs", files, "SDK adapter test fixtures stay out of the distributable mod")
            self.assertNotIn("src/bridge_tests.rs", files)
            self.assertEqual(files["Cargo.lock"], (native / "Cargo.lock").read_bytes())
            for crate in package.CRATES:
                self.assertEqual(files[f"src/deps/{crate}/src/lib.rs"], (root / "crates" / crate / "src/lib.rs").read_bytes())
            adapter = tomllib.loads(files["Cargo.toml"].decode())
            self.assertEqual(adapter["dependencies"]["horizon-mod-api"]["path"], package.SDK_PATH)
            self.assertEqual(adapter["dependencies"]["mmga-neoforge"]["path"], "src/deps/mmga-neoforge")
            self.assertNotIn("workspace", adapter)
            self.assertEqual(package.archive(files), package.archive(package.collect(root)))

    def test_manifest_relocation_and_archive_are_reproducible(self):
        workspace = {
            "package": {"version": "0.1.0", "edition": "2021", "license": "MIT"},
            "dependencies": {"serde": {"version": "1.0", "features": ["derive"]}},
            "lints": {"clippy": {"too_many_arguments": "allow"}},
        }
        original = tomllib.loads('''[package]
name = "mmga-runtime"
version.workspace = true
edition.workspace = true
[dependencies]
mmga-classfile = { path = "../mmga-classfile" }
serde.workspace = true
[dev-dependencies]
zip = "2"
[lints]
workspace = true
''')
        generated = package.standalone(original, workspace, "mmga-runtime")
        self.assertEqual(generated["package"]["version"], "0.1.0")
        self.assertEqual(generated["dependencies"]["serde"], workspace["dependencies"]["serde"])
        self.assertEqual(generated["dependencies"]["mmga-classfile"]["path"], "../mmga-classfile")
        self.assertNotIn("dev-dependencies", generated)
        self.assertEqual(original["package"]["version"], {"workspace": True})
        self.assertEqual(tomllib.loads(package.cargo_toml(generated).decode()), generated)
        files = {"Cargo.toml": package.cargo_toml(generated), "src/lib.rs": b"pub fn original() {}\n"}
        first = package.archive(files)
        self.assertEqual(first, package.archive(files))
        with zipfile.ZipFile(BytesIO(first)) as archive:
            self.assertEqual(archive.namelist(), sorted(files))
            self.assertEqual(archive.read("src/lib.rs"), files["src/lib.rs"])
            self.assertTrue(all(entry.external_attr >> 16 == 0o100644 for entry in archive.infolist()))

    def test_unsafe_paths_dependencies_and_executable_payloads_are_refused(self):
        for name, data in [("../escape.rs", b""), ("src/../escape.rs", b""), ("src/program.exe", b"MZ"), ("src/lib.rs", b"#!/bin/sh")]:
            with self.subTest(name=name), self.assertRaises(ValueError):
                package.validate({name: data})
        for dependency in [{"path": "../../outside"}, {"git": "https://example.invalid/repo"}]:
            with self.subTest(dependency=dependency), self.assertRaises(ValueError):
                package.standalone({"package": {"name": "fixture"}, "dependencies": {"unknown": dependency}}, {}, "fixture")
        with self.assertRaises(ValueError):
            package.standalone({"features": {}, "package": {"name": "fixture"}}, {}, "fixture")

    def test_source_selection_never_follows_links_or_copies_binary_outputs(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "lib.rs").write_bytes(b"pub fn fixture() {}\n")
            (root / "output.jar").write_bytes(b"not Java source")
            self.assertEqual(package.sources(root), [root / "lib.rs"])
            try:
                (root / "link.rs").symlink_to(root / "lib.rs")
            except OSError:
                return  # Windows may not grant unprivileged symlink creation.
            with self.assertRaises(ValueError):
                package.source_bytes(root / "link.rs")


if __name__ == "__main__":
    unittest.main()
