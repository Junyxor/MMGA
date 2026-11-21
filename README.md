# MMGA

A Rust compatibility runtime for running supported, unmodified NeoForge bytecode **inside Horizon**.

The experimental native adapter and package instructions are in [horizon/README.md](horizon/README.md). Build its `.hmod` with `python tools/package_horizon.py`; this is a Horizon native source package, not a Java Minecraft `.jar`. Its current boundary is the single-player integrated-server lifecycle and selected world calls. Guest content registration, networking, Java game mixins and most gameplay events remain unsupported and are explicitly refused when detected.

The current stage-by-stage compatibility record for Yes Steve Model, Create/Ponder, Mekanism, Jade, Curios, Waystones, Sophisticated Backpacks, Applied Energistics 2 and their dependencies on Minecraft 1.21.1 / NeoForge 21.1.266 is in [docs/compatibility.md](docs/compatibility.md). Official jar versions, hashes, licenses and dependencies are recorded in [the test corpus](docs/mod-corpus-2026-11-17.md). Third-party jars are not stored in this repository.

The compatibility record above describes the **standalone probe**, not Horizon gameplay. Successful parsing or a completed modeled lifecycle does not establish JNI, networking, rendering or performance compatibility. Native integration checks and their remaining gaps are recorded separately in [docs/horizon-integration.md](docs/horizon-integration.md).
