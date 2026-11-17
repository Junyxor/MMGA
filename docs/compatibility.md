# 未修改 NeoForge 模组的兼容性记录

同步说明：Windows 续作期间远端另新增 `ca5f00e`。合并时按 Mojang 官方 1.21.1 字节码纠正了其 TextColor 测试：构造器去掉 alpha，`serialize()` 返回名称或 `#RRGGBB` 字符串。该远端提交还引用了未提交的 `RegistryStore.bind_holder`，本轮补齐 deferred holder 的延迟绑定与未绑定访问异常，避免将远端未编译的状态当成已验证基线。

声音值检查点：按官方 jar SHA-1 `30c73b1c5da787909b2f73340419fdf13b9def88` 与 mappings SHA-1 `2244b6f072256667bcd9a73df124d6c58de77992` 实现 Create 使用的 SoundEvent 常量、正确声明类型、静态身份、注册表条目与固定/可变距离。修正 `SHULKER_OPEN` 为 `entity.shulker.open`，未知字段/签名继续失败。三个声音回归测试通过。Create 原 jar 已越过声音表和 `DeferredHolder.create`，合并后复测停在 `CommonComponents.space()`；独立音频播放和渲染尚未实现。

Curios 检查点：修正 `Registries.TRIGGER_TYPE`（单数）与 `BuiltInRegistries.TRIGGER_TYPES`（复数）的精确映射；错误 owner 下的拼写仍报错。Curios 9.5.1 的原始 152 个 class 全部解析，构造完成（3 个配置、8 个监听器），已进入注册，停在 `curios:attribute_modifiers` supplier 的 `DataComponentType.builder()`；setup、tick、玩法、渲染未到达。回归测试先失败后通过。

## 2026-11-17 继续推进（Linux，基线仍是 1.21.1 / NeoForge 21.1.266）

本轮在 `12472a2`（ARGB32）之上实现了 Create 实际读取的原版 `SoundEvents`（按 jar 里的字段类型返回 `SoundEvent` 或 `Holder`）、Mekanism 用到的 `Attributes` 默认值、`TextColor.fromRgb`、`ArmorMaterial` 数据、`ArmorItem.Type.values`，以及 Java 21 的 `List.getFirst` / `getLast`。`DeferredHolder.create(ResourceKey, ResourceLocation)` 会登记一个尚未绑定的 holder，注册同名条目后才能 `get()`。没有为了过探针填默认返回值。

`cargo test --workspace --lib`：classfile 7、runtime 3、NeoForge 21。探针：

```bash
cargo run -p mmga-neoforge --release --example probe -- --dedicated-server /path/to/mod.jar
```

| 模组 | 本轮阶段 | 说明 |
| --- | --- | --- |
| Create 6.0.10 | 仍在构造。比上一记录更深：声音表已建完，停在 `CommonComponents.space()`（`DisplaySource.<clinit>`）。日志已有 `Create 6.0.10 initializing`。注册、setup、tick、玩法、渲染都没到 | Ponder 仍先完成构造 |
| Mekanism 10.7.19 | 仍在构造。已过 `TextColor.fromRgb` 和 `Attributes.ATTACK_SPEED.getDefaultValue`。停在 `ArmorMaterial.defense()`（`MekanismStartupConfig`）。注册及之后未到 | 客户端入口继续被跳过 |
| Jade 15.10.6 | **构造完成，进入注册**。`RegisterPayloadHandlersEvent` 上停在 `StreamCodec$CodecOperation.apply`。setup / tick / 看方块提示 / 渲染未到 | 单独测，JEI 可选未装 |
| AppleSkin 3.0.9 | 加载器跑完到 `Running`：1 条配置、2 个 payload 通道、3 个监听器、1 次回调、31 次加载器事件。这是注册和一次 setup/tick 回调，不是饥饿 HUD | NeoForge 依赖只要求客户端；专用服务器探针仍执行了带 `IEventBus` 的构造 |
| FerriteCore 7.0.3 | 加载器阶段返回成功，0 个注册项、0 个监听器。入口构造函数是空的，效果全在 Mixin，MMGA 没有把它接到真实区块内存上 | 不能当成内存优化已生效 |
| Cloth Config 15.0.140 | 同上，空构造，加载器阶段结束。配置界面是客户端，没有打开 | 库本身没有注册方块 |
| Clumps 19.0.0.1 | 加载器阶段返回成功，0 个监听器。没有观察到经验球合并 | Mixin / 事件若未挂上，就不算玩法 |

新增 jar 只放在本机 `/tmp/mmga-mods`，不进 Git。sha1 与 Modrinth 一致。

| 文件 | 许可证 | sha1 | 来源 |
| --- | --- | --- | --- |
| `ferritecore-7.0.3-neoforge.jar` | MIT | `9563692efb708b6b568df27a01ec52f6311928ef` | https://cdn.modrinth.com/data/uXXizFIs/versions/x7kQWVju/ferritecore-7.0.3-neoforge.jar |
| `cloth-config-15.0.140-neoforge.jar` | GNU LGPLv3 | `c3e5733ba4503b102589a026000fd5ce0212f6f2` | https://cdn.modrinth.com/data/9s6osm5g/versions/izKINKFg/cloth-config-15.0.140-neoforge.jar |
| `appleskin-neoforge-mc1.21-3.0.9.jar` | The Unlicense | `81cf0e668f991f83ac8820c386fbd6c9c3602246` | https://cdn.modrinth.com/data/EsAfCjCV/versions/uAKA6Laj/appleskin-neoforge-mc1.21-3.0.9.jar |
| `Clumps-neoforge-1.21.1-19.0.0.1.jar` | MIT | `fa2576297c7bd12b28aa7070e8141792bd66da4c` | https://cdn.modrinth.com/data/Wnxd13zP/versions/jo7lDoK4/Clumps-neoforge-1.21.1-19.0.0.1.jar |
| `Jade-1.21.1-NeoForge-15.10.6.jar` | CC-BY-NC-SA-4.0 | `88ee316e68900080b017f60c12162e2731924cf8` | https://cdn.modrinth.com/data/nvQzSEkH/versions/eYz2YBGT/Jade-1.21.1-NeoForge-15.10.6.jar |

本轮没有再跑官方 NeoForge 对照，也不比较性能。YSM 的 JNI 缺口维持上一记录，没有假装本地库已加载。

## 2026-11-17 增量复测（Windows x86_64）

本轮先 `git fetch origin`，以远端 `68c50cd` 为基线。该提交以及 `f8005c7`、`0c06e55` 均已存在。本机检查未找到上一轮 checkout 或可恢复的未提交修改。`cargo test --workspace` 基线通过：classfile 7、runtime 3、NeoForge 17，共 27 项；包括 javac 编译的字节码测试。下面的增量记录取代后文 2026-11-15 的旧阻塞描述。

- Mekanism 10.7.19.85：2975 个 class 解析成功；依次修复 `FastColor$ARGB32.color(III)I` 和 `TextColor.fromRgb(I)`。按官方 1.21.1 字节码实现不钳位的 ARGB 位移/按位或，以及 TextColor 去除 alpha、保留 legacy 名称和大写十六进制序列化；两项回归均先失败后通过。原 jar 重跑推进到 `Attributes.ATTACK_SPEED`；仍在构造，注册/setup/tick/玩法/渲染均未到达。
- Create 6.0.10：3805 个 class（含内嵌库）解析成功；Ponder 已完成构造，Create 自身构造已进入，当前阻塞 `SoundEvents.GENERIC_EXPLODE`。组合加载尚未进入注册或 setup；12 个预先登记项不能作为完成注册的证据。tick/玩法/渲染未测试。
- YSM 2.6.5：933 个 class 解析成功；构造、注册事件、common/sided setup 和加载器的一次模拟 tick 返回成功，31 次事件、5 次回调、15 个监听器、1 条命令。YSM 捕获 `System.load` 的 `UnsatisfiedLinkError`，记录 `unsatisfied_runtime_env` WARNING，核心 JNI 初始化失败；0 个模组注册项、0 个网络通道。这里是禁用原生功能后的加载器路径，模型同步、玩法和渲染均未验证，不能称为完整支持。

本轮未进行相同硬件、相同负载的官方 NeoForge 对照，不能据此宣称性能提升。历史服务器数字仅保留作历史观察。

MMGA 的目标是直接加载已经发布的 NeoForge 模组 jar，而不是要求作者按一套桩 API 重写。下面是 2026-11-15 在这台 Linux 机器上的实测。jar 没有放进 Git。

扫描 jar、读出 `mods.toml` 或把 class 解析成功，都不算把模组跑起来。下面的阶段按这个顺序记：元数据、class 加载、构造 `@Mod`、注册、setup、tick、游戏内容或渲染。

## 选定的平台

三个模组的发布说明和 jar 里的 `versionRange` 有一个共同窗口：

| 来源 | Minecraft | NeoForge |
| --- | --- | --- |
| Yes Steve Model 2.6.5（[官方说明](https://ysm.cfpa.team/en/wiki/faq/)） | NeoForge 线为 `1.21.1` 或 `26.1.2` | `[21.0,21.2)`，另有一条 `26.1.2` 构建 |
| Create 6.0.10（jar 内 `neoforge.mods.toml`） | 只接受 `[1.21.1]` | `[21.1.219,)` |
| Mekanism 10.7.19.85（jar 内 `neoforge.mods.toml`） | 只接受 `[1.21.1]` | `[21.1.194,)` |

因此这一轮使用 **Minecraft 1.21.1 + NeoForge 21.1.266**。`21.1.266` 是 [maven.neoforged.net](https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml) 上满足 `[21.1.219,)` 且仍小于 `21.2` 的最新稳定 21.1 版本。YSM 的 `26.1.2` 构建和 Create、Mekanism 不是同一套游戏版本，没有混在一起测。

MMGA 用同一对版本做 `versionRange` 检查（`TARGET_MINECRAFT` / `TARGET_NEOFORGE`）。

## 下载的文件

只从 Modrinth CDN 和 NeoForge Maven 下载。校验值与 Modrinth API 给出的 sha1 一致。

| 文件 | 许可证 | 大小 | sha1 | URL |
| --- | --- | --- | --- | --- |
| `ysm-2.6.5-neoforge+mc1.21.1-release.jar` | All Rights Reserved（Modrinth `LicenseRef-All-Rights-Reserved`） | 63463229 | `18bec74a5b7f3778ce162ddbbbd5555aff0eda2e` | https://cdn.modrinth.com/data/86xjpqqS/versions/HZWaR0LY/ysm-2.6.5-neoforge%2Bmc1.21.1-release.jar |
| `create-1.21.1-6.0.10.jar` | [Create Mod License](https://github.com/Creators-of-Create/Create/blob/HEAD/LICENSE.md)；资源文件另有保留 | 19123767 | `0e97e49837bed766e6f28a4c95b04885d6acc353` | https://cdn.modrinth.com/data/LNytGWDc/versions/UjX6dr61/create-1.21.1-6.0.10.jar |
| `Mekanism-1.21.1-10.7.19.85.jar` | MIT | 11976009 | `b78945c40cfe7640408f3fd1e44da385a8c8b805` | https://cdn.modrinth.com/data/Ce6I4WUE/versions/5KzzycBT/Mekanism-1.21.1-10.7.19.85.jar |
| `neoforge-21.1.266-installer.jar` | NeoForge 安装器，只用于对照基线 | 6992413 | `922e6c9a118144572c2afa0ad0b059671be73837` | https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.266/neoforge-21.1.266-installer.jar |

Create 6.0.10 把依赖打进了 `META-INF/jarjar/`，不需要再单独下载：

- Ponder `1.0.82+mc1.21.1`，`side = BOTH`，必需，MIT
- Flywheel `1.0.6`，`side = CLIENT`，必需，MIT
- Registrate `MC1.21-1.3.0+67`，没有自己的 `mods.toml`，只是库

没有安装、也不该当成启动失败的可选依赖：

- YSM 的 Iris：`optional`，且只在客户端检查
- Create 的 Sodium、Lithium、JourneyMap、Xaero's World Map：可选。Radium、Palladium 被声明为不兼容，但这次没有安装它们
- Mekanism 的 JEI：可选。官方说明里 JEI 能看 Create 的配方，但 Create 6.0.10 的 toml 没有把它列为必需。真实 NeoForge 在没装 JEI 时仍然进入了 `Done`

## 复现 MMGA

```bash
cargo run -p mmga-neoforge --release --example probe -- \
  --dedicated-server /path/to/mod.jar
```

`--client` 只表示加载器按客户端筛选 `@Mod`，不会打开窗口，也不做模型或 Ponder 渲染。

未实现的 `net/minecraft`、`net/neoforged`、`com/mojang`、`org/apache/commons`、`org/apache/logging`、`java/nio` 调用会抛 `UnsupportedOperationException`，不再返回 0 或 null 假装成功。`System.load` / `loadLibrary` 抛 `UnsatisfiedLinkError`。

## 复现 NeoForge 21.1.266 专用服务器

```bash
java -jar neoforge-21.1.266-installer.jar --installServer
printf 'eula=true\n' > eula.txt
mkdir -p mods
# 把上面三个 jar 放进 mods/，不要再加 JEI 或 Iris
printf '%s\n' '-Xmx2G' > user_jvm_args.txt
java @user_jvm_args.txt @libraries/net/neoforged/neoforge/21.1.266/unix_args.txt nogui
```

这次实际命令就是上面这一条，工作目录 `/tmp/mmga-nf-server`。安装器自己下载了 Minecraft 1.21.1 服务端。

## 结果

| 模组 | 侧 | MMGA `--dedicated-server` | 真实 NeoForge 21.1.266 专用服务器 |
| --- | --- | --- | --- |
| Yes Steve Model 2.6.5 | Modrinth：`client_side=optional`，`server_side=optional`。模型界面和渲染是客户端功能；2.2.2 起核心是 C++ 本地库（jar 内有 `libysm-core.so` / `ysm-core.dll`） | 933 个 class 全部解析。元数据和依赖通过。构造函数已进入，停在 `Component.translatable(String, Object[])`。没有注册表阶段，没有模型同步，没有渲染 | 出现在 Mod List。Mixin 应用到 `ServerPlayer` 等类。构造之后注册了事件订阅。服务器 `Done`。没有客户端，不能说明模型菜单或渲染可用 |
| Create 6.0.10 | 项目 API：服务器必需，客户端可选。Flywheel 只在客户端必需 | 3805 个 class（含内嵌 jar）全部解析。Ponder 被识别为独立模组并排在 Create 前面。专用服务器没有把缺 Flywheel 当成错误。Ponder 构造函数停在 `GETSTATIC BuiltInRegistries.COMMAND_ARGUMENT_TYPE`。Create 自己的构造函数还没执行。没有多方块结构，没有 Ponder 界面 | Create 打印 `initializing`，并注入 157 条配方。Ponder、Flywheel 都在 Mod List。服务器 `Done`。没有玩家、没有机械结构，所以没有对应的 tick 负载 |
| Mekanism 10.7.19 | 客户端和服务器都需要 | 2975 个 class 全部解析。客户端入口 `MekanismClient` 在专用服务器上被跳过，公共入口 `mekanism.common.Mekanism` 开始构造。`ModList.isLoaded` 对没安装的 JEI 返回 false，没有把它报成缺依赖。构造停在 `com.mojang.authlib.GameProfile.<init>(UUID, String)`（假人档案）。没有注册、setup 或 tick | 日志有 `Version 10.7.19 initializing...` 和 `Mod loaded.`。服务器 `Done`。没有机器或配方负载 |

真实服务器三者一起启动：`DedicatedServer` 记录 `Done (6.834s)`，出生点准备 `Time elapsed: 2808 ms`。进程在 `Done` 之后约一分钟的 RSS 约为 1.43 GiB（`VmRSS` 1466844 kB，`-Xmx2G`）。没有采集可与 MMGA 对照的 MSPT，因为 MMGA 没有进入带这些模组的世界 tick，真实服务器也没有跑机械动力结构或通用机械机器。空 tick 微基准不能拿来宣称加速。

## 这次改了什么

- Jar-in-Jar 里的 `mods.toml` 会成为独立模组。Create 不再被误报为缺少 Ponder。
- 同一个 mod id 同时有客户端 `@Mod` 和公共 `@Mod` 时，专用服务器只跳过客户端类。以前 Mekanism 会被整包丢掉，加载器却返回成功。
- 构造函数按参数类型注入 `IEventBus`、`ModContainer`、`Dist`，所以 `(ModContainer, IEventBus)` 可以构造。
- `ModList.get` / `getModFileById` / `isLoaded`、`ResourceLocation`、`StringUtil.isNullOrEmpty`、`SystemUtils.OS_ARCH` 按真实数据实现。没实现的 Minecraft / NeoForge 调用直接失败。

## 还没有

- 没有客户端画面。YSM 的模型选择、Create 的 Ponder / Flywheel、Mekanism 的 GUI 都没有跑。
- 没有 `BuiltInRegistries`，没有 `Component` 文本系统，没有 Authlib 的 `GameProfile`，没有方块状态、区块、网络协议。
- 没有把 JNI 本地库加载进 MMGA。YSM 的渲染核心在 `libysm-core` 里。
