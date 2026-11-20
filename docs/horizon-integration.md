# MMGA on Horizon: native-build and validation record

Date: 2026-11-19. Package target: Horizon Nightly 2026-11-18, Minecraft 26.3, commit `0b14f8acfd97fa2c750e42795249dcc981178ca4`, native SDK 1.99.1 and mixins revision `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.

The tested delivery is `dist/mmga-0.1.0-horizon-2026-11-18.hmod` (**584,542 bytes**, SHA-256 `e38bc1c8fd9892551cb32f9510c6193e4cc23afc4eeef7cce401d582a3050b76`). It is a source-native Horizon package, **not** a Java mod JAR, and does not belong in Minecraft Java's `mods/` folder. Recreate the same source package from the repository root using:

```powershell
python tools/package_horizon.py --output dist/mmga-0.1.0-horizon-2026-11-18.hmod
```

## Verified

- Rust adapter checked successfully against the exact pinned Horizon SDK. Native SDK contract tests pass **2/2**.
- Full MMGA workspace tests pass: **368 total**, classfile **9**, runtime **51**, NeoForge **308**, and doc-tests.
- `python -m unittest discover -s tools -p test_package_horizon.py`: **4/4**.
- Horizon's exact pinned `horizon-mod-package::Package::read` accepts the `.hmod` as a native package.
- Horizon launcher's own native builder succeeded using its managed Rust **1.99.1**, `x86_64-pc-windows-gnu`, optimized prebuilt route. It compiled MMGA and the generated patch library, then installed **`native-0b14f8a-71bfcf2f9683`** in a fresh test instance. Every one of the delivery archive's **116 files** matches that build's supplied source folder byte for byte. The native build record uses a source-inventory identity, not the outer ZIP's SHA-256.
- Adapter Clippy exits **0**, with existing core dependency warnings; no warning-free claim. The local SDK contract build uses Rust 1.98.0/MSVC, separately from the launcher's verified Rust 1.99.1/GNU native build. PATH javac is 17.0.12; the tiny contract fixture has Java 17 classfiles and real Minecraft method signatures. The historical official 1.21.1 mod inputs target Java 21.
- Official 26.3 client/server JARs, libraries and non-sound assets were verified by their manifest hashes and imported by the real `horizon-prepare` exporters into an isolated scratch tree. Resource preparation completed successfully. These files and all SDK/runtime caches remain ignored.
- Attempting `horizon-launcher play --instance mmga-isolated-native-test --version <native-build> --world MMGA_Native_Test` stopped before game startup with **“Sign in with a Microsoft account that owns Minecraft: Java Edition to play.”** The isolated launcher has no account. No account was imported or modified, and no login requirement or managed-cache lease check was bypassed.

The [machine-readable result](horizon-build-result-2026-11-19.json) records the shipping hash, build target and verification limits. Full local logs are under `.local/horizon-delivery-evidence/`. Pinned SDK caches, game files and test inputs remain ignored; the temporary runtime test JAR is not in the package.

## Runtime status and exact boundary

The Horizon native `Hooks` adapter loads configured guest JAR paths from the game instance's `config/mmga.toml`, forwards supported lifecycle and tick hooks on a persistent Java worker thread, and sends owned world requests synchronously back to the Horizon host thread. A javac-compiled original-signature fixture exercises real VM server-event calls and the public SDK `LevelRef::set_block` backend in an SDK host-contract harness; it verifies world write/read/time plumbing, `ThreadLocal` continuity, unchanged-write `false`, refusal behavior and panic disconnection.

**The final native build did not enter an actual Horizon world because the isolated launcher requires a Microsoft login.** The fixture was not loaded inside a game world, so visible in-game behavior and gameplay remain unverified. The compiled fixture JAR and its compile-only API classes are not included in the distributed `.hmod`; the adapter's SDK test source is excluded too.

The app-facing environment offers a client native SDK and no Windows dedicated-server target. The current manifest is a client-installed native package, intended to run Java guest server callbacks only in Horizon's integrated single-player server. It is not a standalone or dedicated Horizon server mod. Its explicit guest bridge is limited to canonical propertyless vanilla block states, actual block reads/writes and real game/day time, plus the supported Java loader subset. For the first supported guest example, use a declared propertyless vanilla block (the contract tests use oak planks).

MMGA rejects unsupported Java game-bus event listeners, un-audited nested JARs, mixins, access transformers, coremods, agents, enum extensions, guest mod registry entries, guest network channels and command trees instead of silently dropping them. Its required `registries` capability is part of the Horizon native package schema and is not evidence that guest Java registry content is transferred to Horizon. Property-bearing block states, unimplemented network and general Java-to-Horizon API calls also refuse.

To explicitly configure original guest JARs for the isolated instance:

```toml
# <instance game directory>/config/mmga.toml
jars = ["mmga-mods/guest.jar"]
```

Paths are canonicalized and constrained to that directory. The first configuration-free install loads an empty guest set; it does not scan unrelated folders.

## Reproduce the native build

Extract the source package into an empty folder, install and prepare the pinned Nightly through Horizon, then run its supported client build command:

```text
horizon-launcher native-build <extracted-source-folder> --instance <test-instance> --version nightly-2026-11-18-0b14f8a --trust-native-code
```

The `.hmod` carries dependency pins and feature selections matching that Nightly's prebuilt registry graph. The shared ZIP dependency was upgraded to **9.1.3** because the earlier 2.2.2 dependency introduced a fuzzing-only `arbitrary/derive` edge that Horizon correctly refused. ZIP filename decoding errors now propagate through both the JAR reader and the probe. The core's declared Rust minimum is **1.88**; native builds use the SDK's newer managed toolchain. The minimum version itself was not independently tested.

`horizon-launcher play` requires the operator's Minecraft-owning Microsoft account. Signing in and an actual world run are the remaining manual validation steps; native-build success alone is not a gameplay acceptance result. The historical 17-group probe result must not be presented as a Horizon mod-compatibility result.

## Opt-in source-built 1.21.1 host package

The default command above still produces the same reviewed 26.3 archive. A separate,
explicit package target is available for the locally tested 1.21.1 Horizon port:

```powershell
python tools/package_horizon.py --game-version 1.21.1 `
  --host-revision 0b14f8acfd97fa2c750e42795249dcc981178ca4 `
  --source-revision 2d97cd7e77ae2f7dc9d36d387554cc00cdbca714 `
  --sdk-rustc 1.98.0 --sdk-target x86_64-pc-windows-msvc `
  --output target/horizon-mod/mmga-horizon-1.21.1-dev.hmod `
  --stage-dir target/horizon-mod/stage-1.21.1
```

These values are distinct: `--host-revision` is the native host's **embedded ABI
identity**, while `--source-revision` records the port's source HEAD. This host was
built with Rust **1.98.0/MSVC**. It is not the published Nightly's **1.99.1/GNU** SDK,
and this package does not claim compatibility with that launcher build route.
Changing any identity requires another reviewed target; arbitrary revisions,
compiler versions and target triples are refused. The SDK path in the package
remains a build input supplied by the matching source-built host. The source-only
archive contains no SDK, native library, game resource or Java mod JAR.

The packager first validates the canonical 26.3 manifest pins. It then changes only
the selected archive's host game, target metadata, exact registration-version check
and runtime status. The checkout's 26.3 Rust source is unchanged. Source drift that
removes or duplicates the reviewed version check is refused; the 1.21.1 artifact
continues to reject every other host game version. Metadata is not used to bypass
the registration gate or the host's native build-identity checks.

The actual local 1.21.1 host fixture has exercised the supported lifecycle and
propertyless vanilla block read/write bridge. This does not establish guest
registry registration, payload networking or broad NeoForge-mod compatibility.
Those existing refusals remain in place. A 1.21.1 package target is an explicit
host build selection, not a promise that arbitrary NeoForge 1.21.1 mods will run.
