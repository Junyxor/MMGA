# 未修改 NeoForge 模组的兼容性记录

## 2026-11-17 持续修复增量（进行中）

从 `e192d87` 继续，重新抓取远端且基线 63 项测试通过。已复现并修复 Balm 的 `BlockCapability.createSided`：Block/Entity/Item capability 的工厂保持名称、返回类型及 context class，按种类缓存身份，类型/context 冲突抛异常，`getAll()` 返回不可变快照；NeoForge 自带 energy/fluid/item capability 使用同一身份路径。两项 VM 回归先失败后通过。真实 Balm+Waystones 组现推进到 `MapCodec.unit(Object)`（`NeoForgeBalmWorldGen.<clinit>`），Waystones 仍未构造，注册/setup/tick 未到达。provider 和世界查询尚未实现，继续明确失败。下方表格为上一个完整验证检查点，增量完成后更新。

- GuideME 的 `DeferredRegister.Items.registerItem(name, factory, properties)` 已实现真实延迟 supplier，注册阶段才调用工厂，原 properties 引用及其后续修改保留。两个先失败后通过的 VM 测试覆盖时机、注册对象、null 工厂异常。GuideME 构造完成，AE2 自身服务端构造首次进入，当前停在 `ThreadLocal.<init>()`（`AppEngBase.<init>`）；组合注册/setup 尚未到达。
- Create 使用的 RESPAWN_ANCHOR / TARGET 按官方定义保存真实 registry 值、单方块全部状态（5 / 16 种）、默认状态、charges/power 属性、硬度/抗爆/工具要求/光照。4 项回归验证不可变状态转换和 registry 查找；全局状态 ID、完整方块枚举、未实现形状/世界操作明确失败。原 jar 越过 `Blocks.RESPAWN_ANCHOR`，推进到 Registrate 的 `DeferredHolder.getKey()`；仍在 Create 构造，注册/setup 未到达。
- DeferredHolder 的 key/id/bound 状态与 factory、受保护构造器已保留真实引用，Registrate 子类不丢自身字段。4 项回归覆盖未绑定元数据、注册后绑定、异常和 subclass 构造。Create 越过 `getKey` 与 `DeferredHolder.<init>(ResourceKey)`，下一个阻塞是 `MarkerManager.getMarker`（Registrate `DebugMarkers.marker`）；仍在构造阶段。
- 新增可执行 DFU 8.0.16 子集：JsonOps/DataResult、原始 Codec、ResourceLocation/UUID 四整数编码、字段/optional/default、list/xmap/验证、RecordCodecBuilder getter/group/apply/constructor 与 MapCodec.unit 的真实 JSON 往返和 partial error。7 项回归通过，非 JsonOps、压缩 record map、尚未实现节点明确失败。Sophisticated Core 已实际注册 8 项，当前停在 `ComponentSerialization.CODEC`（`ender_linker_target`）；Curios 推进到 `AttributeModifier.MAP_CODEC`；Balm 越过 unit 后停在 `Tables.synchronizedTable`。三者尚未完成注册/setup。
- StringRepresentable 枚举 codec 已按真实 supplier、名称映射、16 元素阈值、byName 默认值和 compressed ordinal 规则执行，4 项 VM+JSON 回归通过；未知名称返回 DataResult error。Mekanism 越过 `RelativeSide`，推进到 `Function.identity()`（`TransmissionType.<clinit>`）；随后补齐 identity 后再推进至 `DyeColor.BLACK`，仍在构造。
- JDK `Function.identity` 保持对象/null 原样返回；ThreadLocal 按执行线程保存值并支持 initialValue/withInitial/set/remove，未模拟 Thread.start。两个回归先失败后通过，包括跨原生线程的状态隔离。AE2 越过 ThreadLocal 构造，下一阻塞 `SidedThreadGroups.SERVER`；GuideME 构造已完成，注册/setup 未到达。
- Log4j 2.22.1 MarkerManager/Marker 按真实缓存、名称、父链、containment 和异常实现，4 项回归通过。Create 越过 DebugMarkers，推进到 Registrate 的 `Collection.forEach(Consumer)`（HashMultimap 返回的 Set）；日志输出格式和日志级别未被伪造为有效。
- Set 的 `Collection.forEach` 现在实际调用 Consumer，null 消费者、回调异常和结构修改保持真实失败语义，两项回归通过。Create 越过 Registrate.accept 回调分发，下一阻塞为 `BlockEntityType.COMMAND_BLOCK`（`AllDisplaySources.<clinit>`）；仍在构造。
- AttributeModifier 的 id/amount/operation record、三种 operation enum、DFU map codec 和二进制 StreamCodec 按官方语义实现，3 项回归含黄金 JSON/字节及坏输入。Curios 已实际注册首个 `attribute_modifiers` 数据组件，下一阻塞 `IMenuTypeExtension.create(IContainerFactory)`（`curios_container`）；构造完成、注册进行中、setup 尚未到达。
- DyeColor 的 16 个 enum 常量、真实 id/名称/MapColor/texture/firework/text 颜色和查找边界按官方 bytecode 实现，两项全表回归通过。Mekanism 越过 EnumColor，下一阻塞 `Tags$Items.ORES_IRON`（`PrimaryResource.<clinit>`）；构造仍未完成。
- SimpleJsonResourceReloadListener 构造按官方保存 gson/directory（包含 null）并保留父接口；实际资源 prepare/reload 尚未实现并明确失败。两项回归通过。Sophisticated Backpacks 自身构造现已完成，连同 Core 进入注册，4 配置、50 监听器；先推进到 ComponentSerialization stream codec，后续共同注册阻塞见下一增量。

最新验证：**2026-11-17，Windows x86_64，Minecraft 1.21.1 / NeoForge 21.1.266**。实现检查点 `a417383`，Rust `1.98.0`，Java 21。后续文档提交不改变这次探测的代码。三个 crate 仍为 `mmga-classfile`、`mmga-runtime`、`mmga-neoforge`；输入均为官方发布的原始 jar，未修改、未针对桩 API 重编译。

MMGA 还不能替代完整 NeoForge/Minecraft。解析成功不算兼容；加载器到 Running 也不证明网络、世界 tick、玩法或渲染已经可用。下面严格区分执行过的阶段与未到达的阶段。

## 基线、来源与验证

- 开始先 `git fetch origin`，远端起点为 `68c50cd`，包含 `0c06e55`、`f8005c7` 和第三个已推送提交。本机原工作目录为空；定向检查未找到旧 checkout 或可恢复的未提交修改，不能由此断言其他机器的修改永久丢失。
- 工作期间远端另推送 `ca5f00e`，已合并保留。合并时修正其 TextColor alpha 处理及返回类型，补齐遗漏的 `RegistryStore.bind_holder`；后续再修正属性注册、护甲值、枚举身份等语义。
- 基线 `cargo test --workspace`：**27 passed**（classfile 7 / runtime 3 / NeoForge 17）。最终同一命令：**63 passed**（7 / 3 / 53），包括实际 javac 编译的字节码测试，doc-tests 通过。
- `cargo build -p mmga-neoforge --release --example probe --offline` 通过。`cargo clippy --workspace --all-targets --offline` 退出 0，但仍有 lint warnings，包括已有代码建议和新 codec 的 `chunks_exact` 风格建议；未把 lint 警告说成零。本机未单独验证声明的 MSRV 1.83。
- 最终 release 做了 **17 组**真实 jar 探测：原三模组、独立 Ponder、原三组合、五个新增主要模组组、三个依赖独立组，以及四个补充模组。各组实际错误或成功与下表一致。
- [官方测试集来源](mod-corpus-2026-11-17.md) 记录每个文件的版本、官方发布/API URL、SHA-1、许可证、运行端、依赖和 API 引用证据；下载时 SHA-1/SHA-512 均核对。所有第三方 jar 在 Git 忽略的 `.local/mods/`，没有进仓库。
- 新增五个主要热门模组是 **Jade、Curios、Waystones、Sophisticated Backpacks、Applied Energistics 2**；必需依赖为 Balm、Sophisticated Core、GuideME。另复测 FerriteCore、Cloth Config、AppleSkin、Clumps。注册、网络、配置、capability、attachment、数据生成调用覆盖来自真实 bytecode；**本轮未执行独立数据生成模式**。
- 值语义依据为 Mojang 官方 [1.21.1 client jar](https://piston-data.mojang.com/v1/objects/30c73b1c5da787909b2f73340419fdf13b9def88/client.jar) 和 [官方 mappings](https://piston-data.mojang.com/v1/objects/2244b6f072256667bcd9a73df124d6c58de77992/client.txt)，均校验 URL 中的 SHA-1，并用 `javap` 对照；没有提交这两个下载文件。DeferredHolder 异常语义另核对 NeoForge 官方 1.21.1 源码。

## 阶段的含义

“完成”只表示该阶段在本 host 的实际执行返回成功。“流程完成”表示加载器走过此阶段，不能推导该模组必然注册了内容或执行了有意义的回调。`RegisterPayloadHandlersEvent` 在当前加载器的 Registered 阶段之后、CommonSetup 之前发出，因此 Jade 的“注册完成”特指注册表循环，并非网络注册完成。

`tick 1` 是 `tick_server(1)` 的一次**建模 tick**，只推进 `WorldState` 并发出事件；没有实际 Minecraft 区块引擎、玩家/机器负载或客户端连接。每个阶段属于指定的隔离加载组；依赖先失败时，目标模组没有被构造。

## 每个模组的最终状态

| 模组 / 发布版本 | 解析 class | 构造 | 注册 | setup | tick | 玩法 | 渲染 | 下一个阻塞或验证缺口 |
| --- | ---: | --- | --- | --- | --- | --- | --- | --- |
| Mekanism 10.7.19.85 | 2975，通过 | 进行中，失败 | 未到达 | 未到达 | 未到达 | 未验证 | 未验证 | `StringRepresentable.fromEnum(Supplier):EnumCodec`，`mekanism.api.RelativeSide.<clinit>` |
| Create 6.0.10，含内嵌库 | 3805，通过 | Ponder 完成；Create 进行中失败 | 组合未到达 | 未到达 | 未到达 | 未验证 | 未验证 | `Blocks.RESPAWN_ANCHOR:Block`，`AllDisplaySources.<clinit>` |
| Ponder 1.0.82+mc1.21.1，独立 | 470，通过 | 完成 | 完成，1 项 | 流程完成 | 建模 1 次 | 未验证 | 未验证 | 世界/场景/客户端未实现或未执行；没有当前 bootstrap 错误 |
| Flywheel 1.0.6，Create 内嵌 | 含在 Create 扫描，通过 | CLIENT 入口跳过 | 未执行 | 未执行 | 未执行 | 未验证 | 未验证 | 专用服务器探针不能验证 Flywheel 客户端渲染 |
| YSM 2.6.5-neoforge+mc1.21.1 | 933，通过 | 返回完成，**原生功能降级** | 流程完成，0 项 | 流程完成，降级 | 建模 1 次 | 模型同步未验证 | 未验证 | JNI `System.load` 抛 `UnsatisfiedLinkError`，模组捕获后报告运行环境不满足 |
| Jade 15.10.6 | 301，通过 | 完成 | 注册表流程完成；网络注册失败 | 未到达 | 未到达 | 方块查询未验证 | HUD 未验证 | `Block.BLOCK_STATE_REGISTRY:IdMapper`，`BlockAccessorImpl$SyncData.<clinit>` |
| Curios API 9.5.1 | 152，通过 | 完成 | 已进入，首项失败 | 未到达 | 未到达 | 饰品/附件未验证 | 未验证 | `RecordCodecBuilder.create(Function)`，`CurioAttributeModifiers.<clinit>`，注册 `curios:attribute_modifiers` |
| Waystones 21.1.47 + Balm | 387，通过 | 依赖失败，未执行 | 未到达 | 未到达 | 未到达 | 传送未验证 | 未验证 | Balm 的 `BlockCapability.createSided(ResourceLocation,Class)` |
| Balm 21.0.67，独立/依赖 | 605，通过，含 Kuma | 进行中，失败 | 未到达 | 未到达 | 未到达 | capability 未验证 | 未验证 | 同上；Kuma 客户端入口在服务器侧跳过 |
| Kuma API 21.0.8+1.21，Balm 内嵌 | 含在 Balm 扫描，通过 | CLIENT 入口跳过 | 未执行 | 未执行 | 未执行 | 未验证 | 未验证 | 客户端功能未执行 |
| Sophisticated Backpacks 3.26.21.2249 + Core | 381，通过 | Core 完成；Backpacks 进行中失败 | 组合未到达 | 未到达 | 未到达 | 背包未验证 | 未验证 | `SimpleJsonResourceReloadListener.<init>(Gson,String)`，`RegistryLoader.<init>` |
| Sophisticated Core 1.5.38.2484，独立 | 851，通过 | 完成 | 已进入，首项失败 | 未到达 | 未到达 | 未到达 | 未验证 | `Codec.INT:PrimitiveCodec`，注册 `sophisticatedcore:number_of_inventory_slots` |
| Applied Energistics 2 19.2.19 + GuideME | 1825，通过 | 依赖失败，未执行 | 未到达 | 未到达 | 未到达 | 存储网络未验证 | 未验证 | GuideME 的 `DeferredRegister$Items.registerItem(String,Function,Item$Properties)` |
| GuideME 21.1.22，独立/依赖 | 5113，通过 | 进行中，失败 | 未到达 | 未到达 | 未到达 | 书本交互未验证 | 未验证 | 同上 |
| AppleSkin 3.0.9 | 36，通过 | 完成 | 流程完成，0 项，2 通道元数据 | 流程完成 | 建模 1 次 | 饥饿同步未验证 | HUD 未验证 | 没有实际 payload 交换、玩家或客户端 |
| FerriteCore 7.0.3 | 77，通过 | 完成，空入口 | 流程完成，0 项 | 流程完成，0 回调 | 建模 1 次 | Mixin 效果未验证 | 未验证 | 真实区块/状态/模型内存路径未执行，不能声称优化生效 |
| Cloth Config 15.0.140 | 633，通过 | 完成，服务器分支 | 流程完成，0 项 | 流程完成，0 回调 | 建模 1 次 | 配置 GUI 未验证 | 未验证 | 构造器检查 side 并跳过客户端页面注册，并非空构造 |
| Clumps 19.0.0.1 | 15，通过 | 完成，设置 pickup 谓词 | 流程完成，0 项 | 流程完成，0 回调 | 建模 1 次 | 经验球合并未验证 | 未验证 | `ExperienceOrbAccess` / `MixinExperienceOrb` 实体路径未执行 |

原三模组合组：全部解析通过，加载顺序为 **Mekanism → Ponder → YSM → Create**，Mekanism 在同一 `StringRepresentable.fromEnum` 阻塞先退出，后面三个没有构造。单模组成功不能推导组合成功。

Create 内嵌的 Registrate `MC1.21-1.3.0+67` 没有独立 mods.toml，是已纳入解析的库，没有可单独宣称完成的模组生命周期。

补充证据：

- Create 失败时有 12 个登记项、1 配置、88 个监听器，但仍处于构造，不能用计数替代注册阶段完成。Ponder 独立运行为 1 登记项、1 配置、3 命令、5 监听器、32 事件、2 回调。
- YSM 为 `degraded=true`，1 条 `unsatisfied_runtime_env` WARNING；0 模组登记项、0 网络通道、1 命令、15 监听器、31 事件、5 回调。错误包含本机 `ysm-core-2.6.5-neoforge+mc1.21.1.dll` 加载失败。退出 0 是异常被模组自行捕获后的路径，绝非 JNI 已实现。
- Curios 有 3 配置、8 监听器；Core 有 2 配置、20 监听器。二者独立注册均在首项失败，最终模组登记项仍为 0。
- AppleSkin 有 1 配置、2 通道、3 监听器、1 回调、31 事件。FerriteCore/Cloth/Clumps 的登记项、配置、通道、监听器、回调都是 0；流程成功不证明功能生效。
- Clumps 的构造确实执行并保存 `ClumpsCommon.pickupXPEvent`，它本来不在构造器注册监听器。0 监听器不是遗漏入口的证据；真正的 PickupXp 发布和合并需实际经验球/Mixin 路径。

## 本轮实际修复

1. `FastColor.ARGB32` 按 Java 位移/按位或打包；`TextColor` 按原版移除 alpha，保留名称和 `#RRGGBB` 字符串，未知操作失败。
2. SoundEvents、SoundEvent 和 Attributes 保存实际注册值、稳定身份及 Holder$Reference；修正声音 ID、字段类型、范围语义、属性 ID 和浮点精度。
3. DeferredHolder 可以先创建引用再绑定注册值；缺 registry 与未绑定值分别明确抛异常，不能返回 null 蒙混。经 Holder 接口访问也执行相同检查。
4. Curios 使用的 `Registries.TRIGGER_TYPE` 与 `BuiltInRegistries.TRIGGER_TYPES` 按实际 owner 区分。
5. StreamCodec 保存映射、字段 codec、构造器、长度限制等，并实际编码/解码原始类型、VarInt/VarLong、UTF-8、数组、optional、集合和组合。验证了黄金字节、损坏输入、异常传播、递归缓存和静态身份。NBT/registry/DFU 图保留参数，尚未实现的执行明确抛错，不返回假数据。
6. DataComponentType builder 保存 persistent/network codec、cacheEncoding，并创建独立快照；缺失 codec 和 transient 类型使用官方异常。真实 DFU 持久化编码/cache 服务仍未实现。
7. 原版护甲材料完整保存防御映射、数值、音效、repair supplier 和层纹理；层列表不可变，自定义传入列表保持调用方语义。repair supplier 真正执行到 Ingredient 时明确报未实现。
8. CommonComponents.space 返回独立可变组件；Direction 与 ArmorItem.Type 保留规范顺序、对象身份及非法名称异常；EquipmentSlot 常量保持身份，其 values/valueOf 尚未实现。
9. 修复 Object fallback 抢先吞掉 host equals/hashCode/toString 的顺序问题；补齐建模 API 的实际继承关系。回归通过真正 VM 调用与反射检查。
10. probe 在输入失败、依赖失败、构造/注册/setup/tick 失败时返回非零，任何输入 jar 失败都不会偷偷测试剩余子集；输出每个模组构造状态、整体最后完成阶段及降级 warning。

这些属于有限 API 语义与加载进度，不是完整世界、渲染、JNI 或网络协议实现。[历史修复检查点](compatibility-history-2026-11-17.md) 保留每次推进的首个失败点；本页表格是最终状态。

## 复现

```powershell
cargo test --workspace
cargo build -p mmga-neoforge --release --example probe
& target/release/examples/probe.exe --dedicated-server '.local/mods/Mekanism-1.21.1-10.7.19.85.jar'
& target/release/examples/probe.exe --dedicated-server '.local/mods/create-1.21.1-6.0.10.jar'
& target/release/examples/probe.exe --dedicated-server '.local/mods/ysm-2.6.5-neoforge+mc1.21.1-release.jar'
```

跨平台通用命令为 `cargo run -p mmga-neoforge --release --example probe -- --dedicated-server <jar>...`。五个新增主要模组和其依赖的精确命令在[测试集来源文档](mod-corpus-2026-11-17.md#复现命令)。必需依赖需放在同一个命令中；可选集成没有混进隔离组。Ponder 从已验证 Create jar 原样提取，SHA-1 `b10a04f40b9d6d33c346fb38df33f10f7e51b713`。

退出 1 表示 jar/准备/bootstrap/tick 失败；无参数为 2；退出 0 只表示建模流程完成，仍必须看 `degraded` 和具体功能证据。`--client` 仅切换入口筛选，不创建窗口或渲染。本机日志在 `.local/probes/*-final.log`，不提交第三方资源或本机临时文件。

## 官方对照与性能界限

2026-11-15 的历史 Linux 会话曾用官方 NeoForge 21.1.266 与原三模组合组进入 `Done (6.834s)`，记录出生点准备 2808 ms、`-Xmx2G`、约 1.43 GiB RSS。该结果保留于历史文档，本轮没有重做对照，也不能把旧机器数字与当前 Windows MMGA 比较。

**没有相同硬件、相同游戏负载下的官方 NeoForge 对照结果，因此本轮不宣称性能提升。** 空 tick/事件总线/registry lookup 的微基准不是实际游戏加速证据。MMGA 尚无这些大模组的真实机器、多方块、网络同步或客户端渲染负载。
