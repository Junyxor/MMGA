# 未修改 NeoForge 模组的兼容性记录

## 2026-11-17 窗口中断后的恢复验证

已先抓取远端，确认 `1ec1ddf` 与当前分支一致，并保留上一窗口全部未提交修改。首次 workspace 回归为 classfile 7 通过、NeoForge 156 通过 / 1 失败；失败来自尚为桩函数的 `fluid_content`，正在修复，不能把这一轮称为全绿。

- 恢复真实 classfile 的字段/方法反射、字段访问检查及 `Set.addAll`；新增 ImmutableMap 的 keySet/values/entrySet，视图保存源 Map 引用，迭代保留实际条目和重复 value，Collection/Map.Entry/Iterator 的修改路径明确抛异常。两项回归先失败后通过，runtime 共 **14 项通过**。
- 使用新编译的 `fast` 优化探针重跑 GuideME 21.1.22 + AE2 19.2.19：GuideME 构造完成；AE2 已越过 `ImmutableMap.entrySet` 的物品定义阻塞，下一处为 `Method.getParameters()`（`PartModelsHelper.createModels`）。整体仍在构造，注册/setup/tick 未到达，玩法与渲染未验证。本地证据 `.local/probes/ae2-immutable-views.log`，第三方 jar 未修改。
- 继续补齐 `Method.getParameters/getReturnType/invoke`：参数类型来自原 descriptor；invoke 检查数量、receiver、访问权限与拆箱/拓宽转换，真正执行原目标并装箱结果，目标异常保留为 `InvocationTargetException.target/cause`。参数名称与未保留的 MethodParameters 标志仍明确未实现。回归先失败后通过，原 AE2 jar 越过模型方法调用，进入 `AEBlocks`，下一处为 `SoundType.AMETHYST_CLUSTER`；仍在构造，注册/setup/tick 未到达。证据 `.local/probes/ae2-reflection-invoke.log`。
- 恢复并验证中断前的 lazy/either/nonempty codec、注册 alias/subclass、loot function、常量 number provider、registry name codec、Fastutil double map 和 Multimap.putAll。新增 `Codec.orElseGet` 保留真实 supplier，仅在 decode 的 DataResult error 时回退；host 未实现异常和 encode 错误不会被吞掉。
- Create 越过 inherited alias、registry codec、DISPENSER/DROPPER 和 PistonType。两种方块各有 facing × triggered 的 12 个 canonical 状态和官方默认 NORTH/false；世界发射、库存和全局状态 ID 未实现仍失败。当前首个阻塞 `Ingredient.EMPTY`（`TrackMaterialFactory.<init>`），Ponder 已构造、Create 构造未完成，12 已登记项/1 配置/89 监听器，组合注册/setup 未到达。
- `SimpleCriterionTrigger` 初始化和 identity-keyed 玩家 listener map 按真实语义执行，listener 集合调用真实 hash/equals；没有模拟世界 trigger 或发放进度。Curios 已实际注册 **6 项**，完成注册阶段，进入 common setup，停在其自定义 `RegisterCuriosExtensionsEvent` 的 `Event.<init>`；tick/玩法/渲染未验证。
- 流体模块原来的空桩已替换：optional 空栈 JSON `{}` 与 stream VarInt 0、mandatory 空栈异常、amount/真实 registry JSON 值和副本语义经过回归。非空流体网络、非空 component patch、NBT 继续明确失败。ItemContainerContents 保留真实 slot codec 和 256 项边界，空容器 JSON/stream 往返、值相等/不可支持的非空行为均经过验证。Core + Backpacks 均构造完成，实际注册 **47 项**，下一处 `TagKey.codec(ResourceKey)`（`filter_attributes`）；setup/tick 未到达，玩法/渲染未验证。
- Mekanism 越过事件分类和继承 `addAlias` 后仍在构造，9 配置/23 监听器，下一处 `DefaultedRegistry.holderByNameCodec()`（`Chemical.<clinit>`）。未把 holder 元数据或部分注册当作兼容完成。
- 当前恢复检查点 `cargo test --workspace --offline`：**195 项通过**（classfile 7 / runtime 15 / NeoForge 173），doc-tests 全通过。已有 `attribute_modifier::MAP_CODEC` dead-code warning 尚在；声明的 MSRV 1.83 仍未单独验证。测试证据 `.local/probes/resume-workspace-test.log`，优化探测用于加载验证而非性能对比。
- AE2 的四个 amethyst SoundType 已按官方 volume/pitch 1.0 与 break/step/place/hit/fall 原始 SoundEvents 引用实现，11 个声音登记值保持真实 registry 身份；自定义构造器保留 null 和传入引用。两项回归先失败后通过，原 jar 越过 SoundType 常量，下一处为 `BlockBehaviour.Properties.of()`（`AEBaseBlock.defaultProps`），仍在构造。证据 `.local/probes/ae2-sound-types.log`；声音播放未执行。
- Ingredient 的 EMPTY/TagValue/ItemValue/fromValues 保存真实输入和可观察的 Value 数组引用，空 factory 规范化为 EMPTY，枚举 tag 内容和未实现的非空 stack/component 去重明确失败。三项 VM 回归通过（两项在独立临时 harness 先失败），原 Create 越过 TrackMaterialFactory，下一处为 `EnumSet.copyOf(Collection)`（`CommonMetal.<init>`）；仍未完成构造和组合注册，材料匹配/配方玩法未验证。
- `TagKey.codec/hashedCodec` 执行真实 ResourceLocation/xmap/comapFlatMap，普通名字与必须带 `#` 的 hashed 名字分别验证。static/direct/codec 使用同一 (registry,location) interner，四项回归含静态身份先失败后通过。原 Core+Backpacks 从 47 推进到 **60 项**实际注册，下一处 `ExtraCodecs.NON_NEGATIVE_INT`（`CookingLogic.<clinit>`，注册 `recipes_used`）；setup/tick 未到达，tag 内容没有伪造。
- JDK EnumSet 从真实 `values()` universe 构建，保留常量 identity/ordinal 顺序，空 Collection 与空 EnumSet 的 copy 规则分别验证；不可变 wrapper 保留实时 delegate，addAll 的中途异常保留之前修改。四项回归通过，原始 Create jar 的独立 `CommonMetal.<clinit>` 最小复现已通过（17 种金属，真实 EnumSet copy 路径）；完整 Create 在事件目录补齐后推进到 `Tags.Blocks.RELOCATION_NOT_SUPPORTED`，仍未完成构造。
- 事件 host 读取实际 linked 类的 Event/IModBusEvent/ICancellableEvent/abstract 关系，保留 supplied event 原对象，正确匹配具体类与祖先 Consumer；Event 构造和默认取消字段按官方执行。三项 VM 回归、七项 bus 回归及 source catalog 唯一性/完整父链/无环约束通过；实际引用的官方外部事件逐项核对。Curios setup 推进至 FML `ModLoader.postEventWrapContainerInModOrder(Event)`。Mekanism/Create/AE2 先前构造路径均复测恢复；额外取消副作用未实现的交互事件明确拒绝。客户端事件能登记监听器不等于客户端运行或渲染支持。
- BlockBehaviour.Properties 的默认字段、真实 callback/default sound/feature flags、强度与抗爆 clamp、输入 reference、谓词和链式设置按官方 bytecode 实现；MapColor 静态 identity 缺陷经失败回归确认并修复。两个 builder 的状态独立，world predicate 真正调用 state API，不返回猜测结果。两项回归通过，原 AE2 越过 Properties.of 和方块定义，下一处为 `Codec.unboundedMap`（`AEComponents.lambda$static$3`）；仍在构造，注册/setup/tick 未到达。证据 `.local/probes/ae2-block-properties.log`。
- `Registry.holderByNameCodec` 构造真实 DFU 映射，保留 registry，解码调用真实 getHolder，编码检查引用/key/owner/绑定和 registrationInfo；任何尚未实现的引用操作明确失败，不新造一个假 holder。两个新回归先失败后通过、registry codec 共四项通过；原 Mekanism 越过此 codec 构造，下一处 `DefaultedRegistry.createIntrusiveHolder(Object)`（`Chemical.<init>`），仍未构造完成。
- Create 新增的 NeoForge Blocks/Items 和 vanilla BuilderTransformers tag 常量按官方字面量映射，静态/direct 身份一致，不含 tag membership 猜测；四项 tag 回归通过。原 jar 越过 AllBlocks/TagGen，进入 AllItems，下一处 `NeoForgeDataMaps.FURNACE_FUELS`（`ItemBuilder.burnTime`），仍在构造，组合注册/setup 未到达。
- ExtraCodecs.NON_NEGATIVE_INT/POSITIVE_INT 真实整数边界和官方错误信息、static identity、encode 前验证经过失败后通过回归。事件分类收紧暴露并修复 Core/Backpacks 真实引用的外部事件目录缺项后，原始组合已恢复构造完成和 **60 项**注册，越过 NON_NEGATIVE_INT，下一处为 `Codec.unboundedMap`（`CookingLogic.<clinit>`，注册 `recipes_used`）。证据 `.local/probes/backpacks-integer-codec.log`；未宣称整个注册阶段或 setup 完成。
- FML ordered post 真实执行“优先级外层、模组顺序内层”，保留 original event、active container 清理和原异常；三项 VM 回归先失败后通过，回调之后 Minecraft context 按官方回退。Curios 已越过 ordered post，执行 common setup 的排队任务后停在 `EntitySelectorOptions.register`；CommonSetup 尚未完成，最后完成阶段仍为 Registered。Core/Backpacks event-handler lambda 描述符按真实 jar 批量核查，新增目录项均核对官方 source。
- DFU `unboundedMap` 实际执行 key/value codecs，保留不可变 Map、重复解码键错误、partial Map、missed input 和 original remainder，encode 调用真实 Map entries 与子 codec，未知 codec/Java callback 错误继续显式失败。两项新回归先失败后通过，DFU tests 共 20 项通过。Core/Backpacks 已进一步注册 **74 项**，下一处 `BaseFlowingFluid.Properties.<init>`（`xp_still`）；AE2 越过 map 后到 `Codec.dispatchMap`（`AEKey.<clinit>`），仍在构造。
- VM 回调边界补齐：未知 callback、缺失 NativeLambda、null 不再静默返回 null；常规 user callback 按实际参数个数选择函数并经过 VM/Host 调用，多个含糊候选明确失败。两个回归先失败后通过（原路径一处 Rust index panic、一处错误地“成功”返回），runtime 共 **21 项通过**，避免把未执行的 enqueueWork 当作成功。
- NeoForgeDataMaps.FURNACE_FUELS 的 DataMapType metadata 保存真实 id/item registry/persistent-network codec/mandatory=false，FurnaceFuel 构造和正值校验分别按 record/codec 执行。JSON `{burn_time:n}` 与裸整数都实际可读，persistent 写对象、sync 写裸整数；三项回归先失败后通过，未实现数据映射世界/数据生成明确失败。原始 AllItems 最小 loop 已越过 burnTime，完整 Create 在 callback 修复后也越过该点，后续阻塞见增量。
- Create AllItems 的七个 vanilla ItemTags 字段按官方 mappings/字面量映射（含 piglin_loved 与 enchantable 路径），两个回归先失败后通过，same item registry/direct factory 身份一致。完整 Create 越过 AllItems 初始化，当前 `WoodType.OAK`（`AllSpriteShifts.populateMaps`），仍在构造；登记 renderer metadata 不代表实际客户端渲染。
- Canonical registry reference 状态保存真实 intrusive prereference identity、注册绑定、alias/shadow、default/frozen flags，static attribute/sound/block/type holder 与 getHolder 使用同一路径。八项引用回归、四项 codec 回归、七项既有 registry 回归通过，实际 onAdd/onBake 回调保留对象/时机/异常。host 调用不会消费 pending exception，相关 VM 集成回归先失败后通过。Mekanism 越过 intrusive holder，下一处 `Class.getClassLoader()`（`MekanismAPI.<clinit>`），仍在构造。RegistrationInfo 字段保留真实 lifecycle；最终 DFU DataResult 的完整 lifecycle 传播尚未证明与官方完全一致。
- `Codec.dispatchMap` 实际调用类型 getter、MapCodec selector 与字段 codec；encode 按官方两次求 type 的顺序，partial/errors、未知 selector 与 compressed 未实现错误均明确保留。`MapCodec.mapResult` 保留并实际调用原 ResultFunction，通过真实 Json MapLike/RecordBuilder 子集读写条目，encode/decode 都不能绕过 callback。三项回归先失败后通过；AE2 越过 AEKey 初始化，下一处 `Codec.mapResult`（`GenericStack.<clinit>`），仍在构造。
- EntitySelectorOptions 的真实 custom register 保留 modifier/canUse/description，允许官方的覆盖和 null 行为；canUse 只在真实 custom lookup 时执行。四项回归先失败后通过（包括 direct ctor），完整 vanilla option catalog、实体查询和未知 parser 路径明确失败。Curios 越过 selector register，实际完成 **CommonSetup**，下一阶段 InterModProcess 停在 `getIMCStream(Predicate)`；tick/饰品玩法未验证。
- WoodType 的 11 个官方名称与静态身份、registration map 和传入完整 record 构造器字段已验证；builtin 未解析的 BlockSetType/sound/fence 字段和依赖 hash 明确失败。两个回归先失败后通过，原 Create 越过 WoodType 名称初始化，下一处 WeatheringCopper.WeatherState.values；不代表木材声音、世界或渲染已实现。
- WeatherState 的四个 canonical 枚举值、名称/serializedName/ordinal、values 独立数组和 valueOf 异常按官方验证，两项回归先失败后通过。Create 继续推进 sprite metadata，下一处 ItemTags.DOORS（`BuilderTransformers.slidingDoor`）；铜的实际氧化过程未实现。
- `Codec.mapResult` 的原 ResultFunction 现在接收真实 decoded Pair/DataResult、原始 input 和已合并 prefix 的编码结果，unknown callback 明确失败。两个回归先失败后通过，另一个回归复现了 callback 的 stable lifecycle 被丢弃，并修复 top-level codec result 的 lifecycle/input identity 保留。AE2 越过 GenericStack，下一处 ItemStack.SINGLE_ITEM_CODEC（`AECodecs.<clinit>`），仍在构造。
- Jar 解析不再丢弃原始 `.class` resource bytes；ClassLoader 可读取真实原字节而不是把已存在的 class resource 报成缺失。两项回归先失败后通过，涵盖 root/nested 同名 class 的优先级和未 link 的 multi-release/module-info 资源。输入 jar 未被改写；classfile crate 共九项测试通过。
- IMC 以真实 message/supplier refs、事件 container recipient 和延迟 method predicate 遍历实际 queue；消费失败保留当前消息，下游 filter 的删除、toList 前后失败和追加/prefetch 时机经六项回归验证。未加载 forge/fml 别名不再假装已加载。原 Curios 已完成 common/sided setup、InterModProcess 和 **LoadComplete**，服务器命令阶段停在 ItemArgument.item(CommandBuildContext)；饰品玩法与真实 tick 尚未验证。
- Create sliding-door 的 ItemTags.DOORS/TRAPDOORS 使用官方 `minecraft:doors/trapdoors` item registry 键，既有 item-tag 回归新增失败后通过的对应断言。原 jar 越过该处，下一处 `Direction.from2DDataValue`（Catnip Iterate.getHorizontals），仍在构造。
- Direction 的 2D/3D 索引按官方顺序与 Java signed remainder/abs 执行；Axis/AxisDirection 常量和真实 anonymous constant-body superclass metadata 保持身份，EnumSet 对 declaring enum 与 constant-body class 分别校验。五项 Direction 回归和四项 EnumSet 回归通过，原 Create 越过 Iterate 初始化到 BlockTags.SLABS，仍在构造。
- SINGLE_ITEM_CODEC 使用真实 id/components schema，忽略输入 count，并编码原 stack 而不修改其数量；未知 item/component 仍明确失败。一项 VM 回归先失败后通过；AE2 越过该 codec 到 GlobalPos.CODEC（`AEComponents.lambda$static$22`），仍在构造。
- Create CopperBlockSet 的 vanilla SLABS/STAIRS tag 常量按官方 `minecraft:slabs/stairs` block keys 映射，回归先失败后通过。原 Create 越过该处到 ArrayUtils.contains(Object[],Object)；仍未完成构造/注册。
- Commons Lang ArrayUtils.contains/indexOf 执行 wanted.equals(element)，保留 null、start index、live array mutation 和 callback 异常，不使用猜测的 equality。两项回归先失败后通过，原 Create 越过 CopperBlockSet.hasVariant，到 StringUtils.capitalize（Registrate toEnglishName）。
- StringUtils.capitalize 使用固定 OpenJDK 21 Unicode 15.0 的简单 titlecase 映射，不把 Unicode uppercase 的多字符扩展误当 Java titlecase。两个回归先失败后通过，保留 null/空值/未改变引用及非 BMP code point；工具 `tools/GenerateJava21Titlecase.java` 校验官方文件 SHA-256 并验证全部 code points 的重现数据。原 Create 越过 capitalize 到 Collectors.joining；未声称 Rt 已能保存 lone UTF-16 surrogate。
- `Codec.checkRange` 保留真实 min/max，并调用值的 Comparable；lazy错误消息/no partial、exact Long precision、NaN/负零和自定义比较异常由四项 VM 回归加一项 numeric bridge 回归验证。原 Mekanism 越过范围工厂，下一处 Codec.validate(Function)（SerializerHelper）；仍在构造。
- ClassLoader 使用真实 linked class-space identity、解析类和 host classpath bytes；ServiceLoader 改为 metadata/iterator 延迟 provider 构造，检查请求 loader、provider cache/reload 和 constructor cause，不再提前运行所有 provider。八项回归通过，Class.isEnum 区分真实 enum 与匿名 constant body。Mekanism 已越过 MekanismAPI，下一处 `Codec.checkRange(Number,Number)`（SerializerHelper），构造尚未完成。
- 本机工具版本核实：Rust 1.98.0；PATH java/javac 为 17.0.12，javap 及 `D:/MC launcher/Java/21` 中的实际 JDK 为 Microsoft 25.0.3（目录名不是版本证据）。模组输入仍为官方 Java 21 classfile，已由 Rust VM 解析/执行；JDK 21 专有语义依据固定官方源数据，未把本机 Java 工具说成 Java 21，也未在本机运行官方 NeoForge 对照。

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
- ComponentSerialization 的 literal/translatable、fallback/参数、extra、命名/RGB颜色、格式标志、font/insertion 实际 JSON 编解码保持内容，纯字面量按原版简化字符串；不支持的 score/selector/NBT/click/hover 内容明确拒绝。trusted context-free stream codec 保留真实 persistent codec，实际 NBT 未实现仍失败。DFU partial-result、alternative、错误 prefix 和数值转换补充回归均通过。Core 与 Backpacks 现已注册 11 项，下一阻塞 `NeoForgeStreamCodecs.enumCodec(Class)`（`SortBy.<clinit>`）。
- BlockEntityType.COMMAND_BLOCK / ENCHANTING_TABLE 保存真实注册键、holder 和实际构造器 method handle；调用 create 会进入该构造器并在未实现部分明确失败，不返回假实体。两项回归通过；全局未完成 registry 枚举拒绝。Create 显示源初始化推进到 `Util.make(Supplier)`；加入真实 make 回调后进入 AllDisplayTargets，下一阻塞 `BlockEntityType.SIGN`。
- `Util.make(Supplier)` 与 `make(Object,Consumer)` 真正调用传入函数并保留返回对象、null 和异常行为，两项回归通过。Create 当前推进到显示目标静态初始化 `BlockEntityType.SIGN`；不会把 Supplier 未调用算作成功。
- NeoForge 的 54 个明确 item tag 常量按官方 source 固定提交逐项映射 `c:` 与 `neoforge:` 路径，未知名称仍失败，不声称 tag 内容已加载。两项回归通过。Mekanism 越过 `Tags$Items.ORES_IRON`，下一阻塞 `PushReaction.NORMAL`（`BlockResourceInfo.<init>`）。
- 审查补齐每个 DeferredRegister 的 register-event 截止状态：supplier 执行后或 registry 已完成后再添加项目会抛 IllegalStateException，不能接受后永不执行。回归先证明晚注册会静默通过，再修复；deferred 相关 9 项回归通过。
- NeoForgeStreamCodecs.enumCodec 保留 Class 并按真实 VarInt ordinal 编解码，decode 使用真实 enum constants，越界/null/non-enum 异常经过回归。Core 越过 enum codec，下一阻塞 ImmutableMap.Builder 构造，修复该构造后已推进到第 12 项之后的 `CustomData.CODEC`。
- Guava ImmutableMap.Builder 保存 builder 自身状态而非误当 Map，build 的 duplicate/null/keepingLast 和独立快照规则经过两项 VM 回归；生成的 immutable map 拒绝 mutator，未完成的 immutable view 显式拒绝。Core 已注册 12 项，当前 `CustomData.CODEC` 阻塞已继续推进中。
- FML SidedThreadGroups 保存真实 side 元数据与稳定 group 身份；当前线程保持实际 native-thread 身份，未将 loader 主线程假标成 server，未实现 Thread.start。ThreadLocal 子类继承 get/set/remove 也按真实 receiver subtype 调用 initialValue。AE2 越过 SidedThreadGroups，推进到真实 classfile 的反射元数据需求 `Class.getConstructors`。
- IMenuTypeExtension/MenuType 保存真实 factory 和 default FeatureFlagSet，create 才转发 windowId/inventory/buffer；IContainerFactory 的两参默认方法转三参 null 数据不会错误吞参。3 项 VM 回归通过。Curios 现注册 2 项，下一阻塞 `AttachmentType.serializable(Supplier)`；未打开实际菜单或渲染。
- AE2 的构造器与 public 方法反射直接读取 linked 原始 classfile，保留签名、public/default 方法 declaring class；外部祖先元数据不全会明确失败，不伪称 NoSuchMethod。Class 对象按 class-space 名称保持身份。两项回归通过；AE2 越过 GridServices 反射检查，推进到 PartModelsHelper 的 `Class.getDeclaredFields()`；仍未完成构造。
- CustomData/CompoundTag 的已实现 JSON 路径保留 NBT 数值类型、数组/list/compound 和深拷贝；getUnsafe 保留引用，SNBT/binary/未实现实体数据明确失败。ItemStack.EMPTY 与 optional JSON codec 保持空栈规则，已知非空 item 使用真实 registry 和 item prototype；未知组件原型不伪造。Codec.orElse 只处理真实 DataResult error，绝不吞未实现异常，decode remainder 保留原输入。Core/Backpacks 已实际注册 **21 项**，下一阻塞 `Codec.lazyInitialized(Supplier)`（`cooking_inventory`）；玩法/渲染未验证。
- PushReaction 五个值按官方 enum 顺序及身份实现，values/valueOf 与异常两项回归通过。Mekanism 越过方块资源反应初始化，下一阻塞 NoteBlockInstrument.BASEDRUM；不代表活塞行为已实现。
- AttachmentType serializable builder 保留 default supplier、serializer、copy handler 与 flags；read/write/copy 真正调用原始 INBTSerializable 方法，不伪造 NBT。3 项回归验证延迟创建、原对象/holder/provider/tag 传递与异常。Curios 已注册 4 项，下一阻塞为继承的 LootItemConditionalFunction.commonFields；世界实体 attachment 存储路径仍未验证。
- NoteBlockInstrument 的 23 个 enum 常量、三种 instrument type、tunable/custom/above 标志与真实 SoundEvents holder 已按官方验证，两项回归通过。Mekanism 越过 note instrument，下一阻塞 EquipmentSlot.values；没有实现音符播放或渲染。
- EquipmentSlot.values/valueOf 返回官方七个 canonical 槽位的独立数组与正确异常；一项新增回归、armor 共 7 项通过。Mekanism 已进一步到 WorldConfig 的 `DimensionType.MAX_Y`。
- DimensionType 的位数和高度上限常量取自官方推导（12 bits、MIN_Y -2032、MAX_Y 2031、Y_SIZE 4064），一项回归通过。Mekanism 构造已处理 9 个配置和 10 个监听器，暴露后续 event metadata 分类缺陷，继续修复中。
- Create 的 SIGN/LECTERN BlockEntityType 与 vanilla BlockTags（包括 mining/tool tags）已逐项核对，实现 stable type/key/真实factory元数据，一项 tag 回归和两项 type 回归通过。Create 已从显示源/目标进入 AllBlocks/TagGen，当前 Multimap.putAll 阻塞继续修复；全局 tag membership 或真实方块实体行为仍不声称支持。

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
