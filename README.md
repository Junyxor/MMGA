# MMGA

A Rust runtime that aims to load unmodified NeoForge mod jars.

The current stage-by-stage compatibility record for Yes Steve Model, Create/Ponder, Mekanism, Jade, Curios, Waystones, Sophisticated Backpacks, Applied Energistics 2 and their dependencies on Minecraft 1.21.1 / NeoForge 21.1.266 is in [docs/compatibility.md](docs/compatibility.md). Official jar versions, hashes, licenses and dependencies are recorded in [the test corpus](docs/mod-corpus-2026-11-17.md). Third-party jars are not stored in this repository.

This is still a partial runtime. Successful parsing or a completed loader lifecycle does not establish gameplay, JNI, networking, rendering or performance compatibility.
