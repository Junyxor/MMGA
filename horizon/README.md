# MMGA for Horizon — experimental server bridge

This is a native **Horizon `.hmod`**, not a NeoForge `.jar` and not a standalone game. The Horizon launcher compiles the Rust sources in the package against its pinned game build. No JVM or third-party mod JAR is included.

## Pinned target

- Horizon Nightly **2026-11-18**, game **26.3**.
- Horizon commit `0b14f8acfd97fa2c750e42795249dcc981178ca4`.
- Native SDK toolchain **Rust 1.99.1**; the launcher manages it separately.
- Guest API subset: **Minecraft 1.21.1 / NeoForge 21.1.266**.
- **Single-player integrated servers only.** The package installs into a Horizon game client and invokes guest server hooks inside the single-player world's integrated server. It does not run on Horizon dedicated servers, multiplayer clients or Horizon rendering/UI hooks.

The guest and host game versions differ. This bridge maps only the operations listed below and refuses unknown behavior; it does not make arbitrary 1.21.1 mods compatible with 26.3.

## Build the package from this repository

From the MMGA repository root:

```powershell
python tools/package_horizon.py
```

The result is `target/horizon-mod/mmga.hmod`. It contains the adapter and the three existing MMGA crates as source, with standalone Cargo manifests and a registry dependency lock. It excludes the SDK, compiled executables, Minecraft files, Java API stubs and third-party JARs.

## Install and configure

Use the matching Horizon launcher and install/prepare the pinned Nightly first. Import the native package through its native-mod workflow. A `.hmod` is not installed into Java Minecraft's `mods/` directory. Native code requires the launcher's explicit trust setting. Open a single-player world; the integrated server drives MMGA's server-only callbacks.

Place the original Java guest JARs in a subdirectory of the game's **instance directory**, then create `config/mmga.toml` there:

```toml
jars = ["mmga-mods/example.jar"]
```

Only the explicitly listed files are opened, in the supplied order. Paths must be relative, end in `.jar`, remain inside the instance after canonicalization and contain no parent-directory components. Duplicate JARs and unknown config fields are refused. No configuration file means an empty guest set, not automatic scanning or a claim of mod compatibility.

The operator command `/mmga status` reports the Java loader phase, guest count, callback count or the first failure. Runtime failures stop the Java worker; they do not shut down or roll back the whole Horizon server. A guest may have made earlier successful writes before a later callback fails.

## Implemented boundary

- Load Java classes and execute the existing supported construction/setup subset during native registration. Horizon must be running a single-player world because the MMGA hooks need its integrated server.
- Forward actual Horizon server start, pre/post tick and stop hooks in order. Initialization and all Java callbacks use one persistent worker, preserving Java thread identity and `ThreadLocal` state.
- Keep Horizon's borrowed host on its owning thread. A synchronous request/reply channel services guest world calls before returning to Java.
- Resolve canonical native server/level handles; forward game/day time, propertyless modeled block reads and `Level.setBlock` / `setBlockAndUpdate` through the public SDK's real level backend.
- Preserve a host write's `false` result and explicit refusal. Unsupported states, update flags and stale handles fail rather than reporting a simulated success.

Guest registry entries, networking, command trees, ordinary entity/player/block interaction events, Java game mixins, access transformers, coremods, enum extensions and nested JARs are not connected to Horizon. Detected requirements are refused. The package's schema-required `registries` capability does not mean those guest registrations are implemented.

## Validation scope

The official pinned Horizon package reader accepts this package format. The adapter compiles against the exact published SDK source. SDK contract tests execute a compiled Java guest through the real adapter and public SDK handles into a controlled test backend, checking world writes, reads, times, unchanged writes, thread identity and failure paths.

Those tests do not start the game. The independent 17-group NeoForge probe matrix in the repository is also not a Horizon compatibility matrix. Actual launcher build and server-run evidence, when completed, is recorded separately in `docs/horizon-integration.md` in the repository. Major third-party mods such as Create, Mekanism, AE2, YSM and Curios are not advertised as playable on Horizon.
