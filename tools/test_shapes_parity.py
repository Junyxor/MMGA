"""Compare MMGA value geometry against the operator's Minecraft 1.21.1 JAR.

Example (the classpath file contains a Mojmap 1.21.1 JAR and its libraries):
  python tools/test_shapes_parity.py --classpath-file PATH --java JDK/bin/java --javac JDK/bin/javac

Only the original oracle source is tracked. JARs, classfiles, oracle output and
compiler logs stay in the selected private output folder.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT / "tools/fixtures/shapes/ShapeOracle.java"


def checked(command, log, *, env=None, cwd=ROOT):
    with log.open("w", encoding="utf-8") as output:
        result = subprocess.run(command, cwd=cwd, env=env, stdout=output, stderr=subprocess.STDOUT)
    if result.returncode:
        raise RuntimeError(f"Command failed ({result.returncode}); see {log}\n{log.read_text(errors='replace')[-6000:]}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    classpath = parser.add_mutually_exclusive_group(required=True)
    classpath.add_argument("--classpath", help="Mojmap 1.21.1 game JAR and libraries, using the OS path separator")
    classpath.add_argument("--classpath-file", type=Path)
    parser.add_argument("--java", default="java", help="JDK 21+ java executable")
    parser.add_argument("--javac", default="javac", help="JDK 21+ javac executable")
    parser.add_argument("--output", type=Path, default=ROOT / ".local/shapes1211-evidence")
    parser.add_argument("--target-dir", type=Path, help="Optional existing Cargo target directory")
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    cp = args.classpath or args.classpath_file.read_text(encoding="utf-8").strip()
    cp = os.pathsep.join(str(Path(entry or ".").resolve()) for entry in cp.split(os.pathsep))
    try:
        checked([args.javac, "--release", "21", "-classpath", cp, "-d", str(out), str(ORACLE)], out / "oracle-compile.log")
        oracle = out / "oracle.json"
        checked([args.java, "-classpath", str(out) + os.pathsep + cp, "ShapeOracle", str(oracle)], out / "oracle-run.log", cwd=out)
        cases = json.loads(oracle.read_text(encoding="utf-8"))
        if cases["minecraft"] != "1.21.1":
            raise RuntimeError("Oracle did not run Minecraft 1.21.1")
        env = {**os.environ, "MMGA_SHAPE_ORACLE_JSON": str(oracle)}
        command = ["cargo", "test", "-p", "mmga-neoforge"]
        if args.target_dir:
            command += ["--target-dir", str(args.target_dir.resolve())]
        checked(command + ["shapes::tests::", "--", "--include-ignored"], out / "oracle-compare.log", env=env)
        checked(command + ["rotation::tests::", "--", "--include-ignored"], out / "rotation-oracle-compare.log", env=env)
        checked(command + ["direction_ops::tests::", "--", "--include-ignored"], out / "direction-oracle-compare.log", env=env)
        print(f"PASS Minecraft 1.21.1: {len(cases['cases'])} box inputs, "
              f"{len(cases['joins'])} joins, {len(cases['collisions'])} collisions, "
              f"{len(cases['moves'])} moves/optimizations, reference identities and "
              f"{len(cases['rotations'])} rotations (196 integer cases), "
              f"{len(cases['directions'])} direction transforms. Evidence: {out}")
        return 0
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(error)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
