# 2026-11-17 官方 jar 测试集来源

目标固定为 Minecraft **1.21.1** / NeoForge **21.1.266**。所有文件均为发布者在 Modrinth 发布的未修改 jar；通过 Modrinth API 查询 `loaders=["neoforge"]`、`game_versions=["1.21.1"]` 后固定到下面的版本 ID，再核对 jar 内 `META-INF/neoforge.mods.toml` 的实际依赖范围。下载文件的 **SHA-1 和 SHA-512 均与版本 API 相符**。文件放在被 Git 忽略的 `.local/mods/`，API 响应在 `.local/corpus-metadata/`，运行日志在 `.local/probes/`；这些二进制及本地缓存不随仓库分发。

此文件记录测试输入及首次探测，不是兼容性承诺。后续修复后的各阶段状态以 [compatibility.md](compatibility.md) 为准。数据生成 API 的覆盖依据是发布 jar 中的真实引用；本轮普通服务器探测没有执行数据生成启动模式。

## 原有三个模组与 Ponder

- Yes Steve Model **2.6.5-neoforge+mc1.21.1**，版本 [HZWaR0LY](https://api.modrinth.com/v2/version/HZWaR0LY)，[官方文件](https://cdn.modrinth.com/data/86xjpqqS/versions/HZWaR0LY/ysm-2.6.5-neoforge%2Bmc1.21.1-release.jar)。文件名 `ysm-2.6.5-neoforge+mc1.21.1-release.jar`，大小 63,463,229 bytes，SHA-1 `18bec74a5b7f3778ce162ddbbbd5555aff0eda2e`。项目 API 标记客户端和服务器均 optional；许可证 All Rights Reserved。JNI 与客户端渲染另需验证。
- Create **6.0.10+mc1.21.1**，版本 [UjX6dr61](https://api.modrinth.com/v2/version/UjX6dr61)，[官方文件](https://cdn.modrinth.com/data/LNytGWDc/versions/UjX6dr61/create-1.21.1-6.0.10.jar)。文件名 `create-1.21.1-6.0.10.jar`，大小 19,123,767 bytes，SHA-1 `0e97e49837bed766e6f28a4c95b04885d6acc353`。项目 API 标记客户端 optional、服务器 required；[Create Mod License](https://github.com/Creators-of-Create/Create/blob/HEAD/LICENSE.md)。依赖和内嵌版本见兼容性主文档。
- Mekanism **10.7.19.85**，版本 [5KzzycBT](https://api.modrinth.com/v2/version/5KzzycBT)，[官方文件](https://cdn.modrinth.com/data/Ce6I4WUE/versions/5KzzycBT/Mekanism-1.21.1-10.7.19.85.jar)。文件名 `Mekanism-1.21.1-10.7.19.85.jar`，大小 11,976,009 bytes，SHA-1 `b78945c40cfe7640408f3fd1e44da385a8c8b805`。客户端/服务器均 required，MIT。
- Ponder **1.0.82+mc1.21.1** 从已验证的 Create jar 的 `META-INF/jarjar/ponder-neoforge-1.0.82+mc1.21.1.jar` 原样提取；没有重新编译或修改。独立文件 SHA-1 `b10a04f40b9d6d33c346fb38df33f10f7e51b713`，MIT。jar 要求 Minecraft `[1.21.1]`、NeoForge `[21.1.206,)`，Flywheel `[1.0.0,2.0)` 仅 CLIENT 必需。独立服务器探测不需要另装 Flywheel。

## 新增五个主要模组

热门程度取自查询时 Modrinth 项目 API 的累计下载量，仅用于说明选样依据，并不是跨平台总下载量。

### Jade

- [项目 API](https://api.modrinth.com/v2/project/nvQzSEkH)，累计下载 74,351,791；[官方源码](https://github.com/Snownee/Jade)。
- 固定版本 **15.10.6+neoforge**，发布于 2026-08-06，版本 [eYz2YBGT](https://api.modrinth.com/v2/version/eYz2YBGT)。
- [官方 jar](https://cdn.modrinth.com/data/nvQzSEkH/versions/eYz2YBGT/Jade-1.21.1-NeoForge-15.10.6.jar)：`Jade-1.21.1-NeoForge-15.10.6.jar`，726,853 bytes，SHA-1 `88ee316e68900080b017f60c12162e2731924cf8`。
- 运行端：客户端/服务器均 optional；服务端提供查询数据，HUD 渲染仍需客户端。许可证 **CC-BY-NC-SA-4.0**，项目 API 与 jar 一致。
- 依赖：jar 要求 NeoForge `[21.0.143, )`；API 声明 JEI optional，本次没有安装 JEI。Minecraft 版本由版本 API 的 `1.21.1` 标签确认，jar 未单独声明 Minecraft 范围。
- API 覆盖依据：`snownee/jade/util/CommonProxy.class` 引用 `RegisterPayloadHandlersEvent`、`BlockCapability`、`EntityCapability`；`snownee/jade/network/ServerPingPacket.class` 引用 `StreamCodec.CodecOperation`。
- 首次服务器探测：301 个 class 全部解析；构造完成、注册阶段完成（0 个模组注册条目）；随后在网络注册事件失败，未完成 common setup。错误为 `StreamCodec$CodecOperation.apply(StreamCodec)` 未实现，发生于 `ServerPingPacket.<clinit>`。没有运行 tick、玩法或 HUD。

### Curios API

- [项目 API](https://api.modrinth.com/v2/project/vvuO3ImH)，累计下载 33,840,113；[官方源码](https://github.com/TheIllusiveC4/Curios)。
- 固定版本 **9.5.1+1.21.1**，发布于 2025-05-14，版本 [yohfFbgD](https://api.modrinth.com/v2/version/yohfFbgD)。
- [官方 jar](https://cdn.modrinth.com/data/vvuO3ImH/versions/yohfFbgD/curios-neoforge-9.5.1%2B1.21.1.jar)：`curios-neoforge-9.5.1+1.21.1.jar`，410,690 bytes，SHA-1 `418fcd42e3a7844c9bdc71c9b6401fdb3894e0c4`。
- 运行端：客户端/服务器均 required。许可证 **LGPL-3.0-or-later**，项目 API、jar 元数据和 jar 内 `LICENSE` 一致。
- 依赖：NeoForge `[21.1.60,)`、Minecraft `[1.21, 1.22)`；没有额外必需模组。
- API 覆盖依据：`Curios.class` 引用 `RegisterCapabilitiesEvent` 和 `RegisterPayloadHandlersEvent`；`common/CuriosRegistry.class` 使用 DeferredRegister 注册附件、触发器、菜单、命令参数和数据组件；`common/capability/CurioInventoryCapability.class` 引用 `AttachmentType`。客户端配置类引用 `ModConfigSpec`。
- 首次服务器探测：152 个 class 全部解析；构造在 `CuriosRegistry.<clinit>` 的 `Registries.TRIGGER_TYPE` 静态字段失败；注册、setup、tick、玩法、渲染未到达。

### Waystones

- [项目 API](https://api.modrinth.com/v2/project/LOpKHB2A)，累计下载 27,966,370；[官方源码](https://github.com/TwelveIterations/Waystones)。
- 固定版本 **21.1.47+neoforge-1.21.1**，发布于 2026-11-15，版本 [sE6fMBGR](https://api.modrinth.com/v2/version/sE6fMBGR)。
- [官方 jar](https://cdn.modrinth.com/data/LOpKHB2A/versions/sE6fMBGR/waystones-neoforge-1.21.1-21.1.47.jar)：`waystones-neoforge-1.21.1-21.1.47.jar`，1,254,420 bytes，SHA-1 `51c3390c2555416979905f9d1b05ebc1a4de29c8`。
- 运行端：客户端/服务器均 required。许可证 **All Rights Reserved**；jar 的 `LICENSE_Waystones` 另标注 returnScroll/warpStone 图标为 CC-BY-SA 3.0。发布者的[授权页](https://mods.twelveiterations.com/permissions)允许在支持的平台用于整合包；本仓库不重新分发这些文件。
- 依赖：NeoForge `[21-beta,)`、Minecraft `[1.21,)`、**Balm `[21.0.59,)`**。使用下面的 Balm 21.0.67 满足范围。版本 API 也确认 NeoForge / 1.21.1 构建。
- API 覆盖依据：Waystones 经 Balm 抽象访问注册、配置和网络；`compat/RepurposedStructuresIntegration.class` 直接引用 DeferredRegister。Balm 的确切 NeoForge 调用见下文。
- 首次服务器探测：Waystones 387 个 class 全部解析，依赖 Balm 605 个 class 全部解析；Balm 的构造在 `BlockCapability.createSided(ResourceLocation,Class)` 失败。Waystones 自己的构造尚未执行；注册、setup、tick、传送玩法、渲染未到达。

### Sophisticated Backpacks

- [项目 API](https://api.modrinth.com/v2/project/TyCTlI4b)，累计下载 21,052,288；[官方源码](https://github.com/P3pp3rF1y/SophisticatedBackpacks)。
- 固定发布版本 **1.21.1-3.26.21.2249**，jar 内模组版本 **3.26.21**，发布于 2026-11-09，版本 [tJnK2DE1](https://api.modrinth.com/v2/version/tJnK2DE1)。
- [官方 jar](https://cdn.modrinth.com/data/TyCTlI4b/versions/tJnK2DE1/sophisticatedbackpacks-1.21.1-3.26.21.2249.jar)：`sophisticatedbackpacks-1.21.1-3.26.21.2249.jar`，1,265,683 bytes，SHA-1 `940e93e9eaaf7db50c308c2a849a471830f6c1ea`。
- 运行端：客户端/服务器均 required。许可证 **All Rights Reserved**，项目 API 与 jar 一致。
- 依赖：NeoForge `[21.1.229,)`、Minecraft `[1.21.1,1.21.2)`、**Sophisticated Core `[1.5.32,)`**。使用下面的 Core 1.5.38 满足范围。Curios、JEI 等集成是 optional，本组隔离探测不安装它们。
- API 覆盖依据：`SophisticatedBackpacks.class` 引用 `GatherDataEvent`；`init/ModItems.class` 引用 DeferredRegister、`RegisterCapabilitiesEvent`、`ItemCapability`、`AttachmentType`；`init/ModPayloads.class` 引用 `RegisterPayloadHandlersEvent`；`Config$Server.class` 引用 `ModConfigSpec`。
- 首次服务器探测：Core 851 个 class、Backpacks 381 个 class 全部解析。Core 构造完成，存储了两个配置；Backpacks 自己的构造进入后，停在 `SimpleJsonResourceReloadListener.<init>(Gson,String)`，调用方 `registry.RegistryLoader.<init>`。合组未进入注册、setup、tick、背包玩法、渲染。

### Applied Energistics 2

- [项目 API](https://api.modrinth.com/v2/project/XxWD5pD3)，累计下载 6,405,104；[官方源码](https://github.com/AppliedEnergistics/Applied-Energistics-2)。
- 固定版本 **19.2.19**，发布于 2026-11-09，版本 [DxZ2waV0](https://api.modrinth.com/v2/version/DxZ2waV0)。
- [官方 jar](https://cdn.modrinth.com/data/XxWD5pD3/versions/DxZ2waV0/appliedenergistics2-19.2.19.jar)：`appliedenergistics2-19.2.19.jar`，8,238,387 bytes，SHA-1 `aa28d9d98d400fa9a191456bb14a433a3a2f6bfc`。
- 运行端：客户端/服务器均 required。项目 API 许可证为 **LicenseRef-Multiple**；[该发布标签的许可证说明](https://github.com/AppliedEnergistics/Applied-Energistics-2/blob/da9bd7f09a94272a67f9825e4c0f4f6b61983b69/README.md#license)区分 API MIT、主体 LGPLv3、纹理/模型 CC-BY-NC-SA 3.0、文字/翻译 CC0、部分音效 CC-BY 4.0，不能把整个 jar 简写成 MIT。
- 依赖：NeoForge `[21.1.169,)`、Minecraft `[1.21.1]`、**GuideME `[21.1.1,)`**。使用下面的 GuideME 21.1.22。EMI、Jade、The One Probe 等为 optional；VanillaFix 被标为 discouraged，本次未安装。
- API 覆盖依据：`appeng/core/AppEngBase.class` 引用 `RegisterPayloadHandlersEvent` 和 `AttachmentType`；`appeng/init/InitCapabilityProviders.class` 引用 `RegisterCapabilitiesEvent` 和 `ItemCapability`；`appeng/datagen/AE2DataGenerators.class` 引用 `GatherDataEvent`；`appeng/core/AEConfig.class` 引用 `ModConfigSpec`；`appeng/init/worldgen/InitStructures.class` 引用 DeferredRegister。
- 首次服务器探测：GuideME 5,113 个 class、AE2 1,825 个 class 全部解析；依赖 GuideME 的构造停在 `DeferredRegister$Items.registerItem(String,Function,Item$Properties)`。AE2 服务端入口 `appeng/core/AppEngServer` 已选中但尚未构造；注册、setup、tick、存储网络玩法、渲染未到达。

## 必需依赖的来源与独立探测

### Balm 21.0.67

- 版本 [gMedgtzL](https://api.modrinth.com/v2/version/gMedgtzL)，[项目 API](https://api.modrinth.com/v2/project/MBAkmtvl)，[官方 jar](https://cdn.modrinth.com/data/MBAkmtvl/versions/gMedgtzL/balm-neoforge-1.21.1-21.0.67.jar)。
- 文件 `balm-neoforge-1.21.1-21.0.67.jar`，786,010 bytes，SHA-1 `68c95ca8a6a87d26199d43ff6978bc94949329e3`。客户端/服务器均 optional（对 Waystones 则为必需依赖），All Rights Reserved，jar `LICENSE_Balm` 链接同一[授权页](https://mods.twelveiterations.com/permissions)。
- jar 要求 NeoForge `[21.0.82-beta,)`、Minecraft `[1.21.1,)`。内嵌 `kuma-api-neoforge-21.0.8+1.21.jar`；其入口是 CLIENT，在专用服务器上被跳过。
- `neoforge/NeoForgeBalm.class` 引用 `BlockCapability`/`ItemCapability`；`neoforge/capability/NeoForgeBalmCapabilities$Registrations$1.class` 引用 `RegisterCapabilitiesEvent`；`neoforge/network/NeoForgeBalmNetworking$Registrations.class` 引用 `RegisterPayloadHandlersEvent`；`neoforge/config/LoadedNeoForgeConfig.class` 引用 `ModConfigSpec`；`api/BalmRuntime.class` 引用 `AttachmentType`。
- 独立探测仍在构造失败于 `BlockCapability.createSided(ResourceLocation,Class)`。之后的注册、setup、tick、玩法、渲染均未到达。

### Sophisticated Core 1.5.38.2484

- 固定发布版本 **1.21.1-1.5.38.2484**，jar 内版本 **1.5.38**，版本 [hhok4NTk](https://api.modrinth.com/v2/version/hhok4NTk)，[项目 API](https://api.modrinth.com/v2/project/nmoqTijg)，[官方 jar](https://cdn.modrinth.com/data/nmoqTijg/versions/hhok4NTk/sophisticatedcore-1.21.1-1.5.38.2484.jar)。
- 文件 `sophisticatedcore-1.21.1-1.5.38.2484.jar`，1,855,171 bytes，SHA-1 `d03c42d2f00ddec0bf999b3525afe85dba6c6e46`。客户端/服务器均 required，All Rights Reserved。
- jar 要求 NeoForge `[21.1.229,)`、Minecraft `[1.21.1,1.21.2)`。JEI `[19.32.0.359,)` 仅客户端 optional。
- `data/DataGenerators.class` 引用 `GatherDataEvent`；`init/ModPayloads.class` 引用 `RegisterPayloadHandlersEvent`；`Config.class` 引用 `ModConfigSpec`；`util/CapabilityHelper.class` 引用物品/方块/实体 capability；`init/ModRecipes.class` 引用 DeferredRegister。
- 独立探测完成构造，注册阶段在 `sophisticatedcore:number_of_inventory_slots` 的 supplier 失败：`DataComponentType$Builder.<init>()V` 未实现，调用方 `ModCoreDataComponents.lambda$static$0`。配置数 2、监听器数 20；setup、tick、玩法、渲染未到达。

### GuideME 21.1.22

- 版本 [QeYKsstN](https://api.modrinth.com/v2/version/QeYKsstN)，[项目 API](https://api.modrinth.com/v2/project/Ck4E7v7R)，[官方 jar](https://cdn.modrinth.com/data/Ck4E7v7R/versions/QeYKsstN/guideme-21.1.22.jar)。
- 文件 `guideme-21.1.22.jar`，9,332,340 bytes，SHA-1 `eb2f531c91566a0f119b4b3a3acd927b48b6b27e`。客户端/服务器均 required。
- 项目 API 许可证为 **LicenseRef-Multiple-OSS-Licenses**；[该发布标签的官方许可证文件](https://github.com/AppliedEnergistics/GuideME/blob/3e09d44294799bcf29adb61a2613db7578313feb/LICENSE.md)包含 LGPLv3，jar 内另有 `META-INF/LICENSE.txt`（Apache-2.0，包含依赖的许可证）。不把整个组合 jar 简化成单一许可证。版本标签 `v21.1.22` 指向 `3e09d44294799bcf29adb61a2613db7578313feb`。
- jar 要求 NeoForge `[21.1.1,)`、Minecraft `[1.21.1]`，没有其他必需模组。
- `guideme/internal/GuideME.class` 引用 DeferredRegister 和 `RegisterPayloadHandlersEvent`；`GuideMEClient.class` 引用 `GatherDataEvent`；`GuideMEClient$ClientConfig.class` 引用 `ModConfigSpec`。客户端入口在专用服务器上跳过。
- 独立探测同样在构造停于 `DeferredRegister$Items.registerItem(String,Function,Item$Properties)`；注册、setup、tick、书本内容交互、渲染未到达。

## 复现命令

以下 PowerShell 命令在仓库根目录执行。一次只加载一个主要模组及其必需依赖，避免可选集成改变首个失败点。

```powershell
cargo build -p mmga-neoforge --release --example probe
& target/release/examples/probe.exe --dedicated-server '.local/mods/Jade-1.21.1-NeoForge-15.10.6.jar'
& target/release/examples/probe.exe --dedicated-server '.local/mods/curios-neoforge-9.5.1+1.21.1.jar'
& target/release/examples/probe.exe --dedicated-server '.local/mods/balm-neoforge-1.21.1-21.0.67.jar' '.local/mods/waystones-neoforge-1.21.1-21.1.47.jar'
& target/release/examples/probe.exe --dedicated-server '.local/mods/sophisticatedcore-1.21.1-1.5.38.2484.jar' '.local/mods/sophisticatedbackpacks-1.21.1-3.26.21.2249.jar'
& target/release/examples/probe.exe --dedicated-server '.local/mods/guideme-21.1.22.jar' '.local/mods/appliedenergistics2-19.2.19.jar'
& target/release/examples/probe.exe --dedicated-server '.local/mods/ponder-neoforge-1.0.82+mc1.21.1.jar'
```

首次结果使用远端起点 `68c50cd` 构建的 release probe。Ponder 独立运行的 trace 为 `discovered → constructed → registered → common-setup → sided-setup → load-complete → server-started → running`，有 1 个模组注册条目、1 个配置、3 个命令、5 个监听器，无 loading issue；`tick_server(1)` 返回成功。但这是 MMGA 的建模生命周期和单次建模 tick，不证明 Ponder 客户端场景、世界模拟或渲染可用，也没有构成与官方 NeoForge 的性能对照。
