# 未修改 NeoForge 模组的兼容性记录

验证日期：**2026-11-18**；平台：Windows x86_64；目标为 **Minecraft 1.21.1 / NeoForge 21.1.266**。
实现检查点：`5be16a611796a74a36a1a0ff5ebbf59ea80e6e6d`。后续文档提交不改变这次探测的代码。

**17 组原始 JAR 探测中，8 组到达 Running 并完成一次建模 tick；其中 YSM 为原生功能降级路径。** 这些结果不代表完整模组兼容。
逐组退出码、构造状态和错误摘要见[本次机器可读结果](probe-results-2026-11-18.json)；固定版本、官方来源、依赖、许可证和 SHA-1/SHA-512 校验见[输入来源记录](mod-corpus-2026-11-17.md)。此前检查点已归档至[历史记录](compatibility-history-2026-11-17.md)。

## 验证范围与工具

- `cargo test --workspace --offline`：**363 项通过**（classfile 9 / runtime 51 / NeoForge 303），doc-tests 全通过。
- `cargo build -p mmga-neoforge --profile fast --offline --example probe` 通过；`cargo clippy --workspace --all-targets --offline` 退出 0，仍有 dead-code 和 lint warnings，不是零警告结果。
- Rust 为 **1.98.0**；声明的 MSRV 1.83 本轮未单独验证。PATH `javac` 为 **17.0.12**；用于检查 bytecode 的 `D:/MC launcher/Java/21` 实际为 Microsoft JDK **25.0.3**，目录名不代表版本。
- 模组输入仍为面向 **Java 21** 的官方 classfile，由 Rust VM 解析和执行；没有修改第三方 JAR，也没有把模组针对桩 API 重新编译。测试中的 javac 小型 fixture 与原始模组探测分别记录。
- 精确原始日志位于忽略目录 `.local/probes/verified-5be16a6-*.log`；测试与 clippy 日志为 `checkpoint-workspace-test.log`、`checkpoint-clippy.log`。第三方 JAR、官方对照二进制和本地缓存不提交。

## 阶段的含义

“完成”仅表示指定加载组中的该阶段实际返回成功；“流程完成”不说明模组注册了有意义的内容或执行了回调。计数是整个加载组的累计值，不能拆成未记录的逐模组贡献。
`Registered` 表示注册表循环完成；其后的 `RegisterPayloadHandlersEvent` 仍可能失败，因此 Jade 的网络注册尚未完成。setup 包括 common/sided setup、IMC 和 load-complete 流程。
`tick 1` 是 `tick_server(1)` 推进 `WorldState` 并发出事件的一次**建模 tick**，没有实际 Minecraft 区块引擎、玩家、机器或客户端连接。各行玩法与渲染均需另行验证。

## 当前逐模组状态

| 模组 / 发布版本 | 解析 class | 构造 | 注册 | setup | tick | 玩法 | 渲染 | 首个阻塞或验证缺口 |
| --- | ---: | --- | --- | --- | --- | --- | --- | --- |
| Mekanism 10.7.19.85 | 2975，通过 | 进行中，失败 | 未到达 | 未到达 | 未到达 | 未验证 | 未验证 | `Block.box(DDDDDD)`，`BlockShapes.box` |
| Create 6.0.10，含内嵌库 | 3805，通过 | Ponder 完成；Create 失败 | 组合未到达 | 未到达 | 未到达 | 未验证 | 未验证 | `BlockTags.DRIPSTONE_REPLACEABLE`，`CreateRegistrate.paletteStoneBlock` |
| Ponder 1.0.82+mc1.21.1，独立 | 470，通过 | 完成 | 完成，1 项 | 流程完成 | 建模 1 次 | 未验证 | 未验证 | 场景、世界与客户端未执行 |
| Flywheel 1.0.6，Create 内嵌 | 含于 Create，通过 | CLIENT 入口跳过 | 未执行 | 未执行 | 未执行 | 未验证 | 未验证 | 专用服务器探针未执行客户端渲染 |
| YSM 2.6.5-neoforge+mc1.21.1 | 933，通过 | 完成，原生功能降级 | 流程完成，0 项 | 流程完成，降级 | 建模 1 次 | 模型同步未验证 | 未验证 | JNI 加载失败，模组捕获并报告 `unsatisfied_runtime_env` |
| Jade 15.10.6 | 301，通过 | 完成 | 注册表完成；网络注册失败 | 未到达 | 未到达 | 方块查询未验证 | HUD 未验证 | `Block.BLOCK_STATE_REGISTRY` 与完整协议状态 ID |
| Curios API 9.5.1 | 152，通过 | 完成 | 完成，6 项 | 流程完成 | 建模 1 次 | 饰品/附件未验证 | 未验证 | 17 条通道元数据；未交换真实 payload |
| Waystones 21.1.47 + Balm | 387，通过 | 依赖 Balm 失败，未执行 | 未到达 | 未到达 | 未到达 | 传送未验证 | 未验证 | Balm 的 `Tables.synchronizedTable(Table)` |
| Balm 21.0.67，独立/依赖 | 605，通过，含 Kuma | 进行中，失败 | 未到达 | 未到达 | 未到达 | 未验证 | 未验证 | `Tables.synchronizedTable(Table)`，`NeoForgeBalmEvents.<init>` |
| Kuma API 21.0.8+1.21，Balm 内嵌 | 含于 Balm，通过 | CLIENT 入口跳过 | 未执行 | 未执行 | 未执行 | 未验证 | 未验证 | 客户端入口未执行 |
| Sophisticated Backpacks 3.26.21.2249 + Core | 381，通过 | 两者完成 | 已注册 74 项，失败 | 未到达 | 未到达 | 背包未验证 | 未验证 | 注册 `sophisticatedcore:xp_still` 时 `BaseFlowingFluid` 构造未实现 |
| Sophisticated Core 1.5.38.2484，独立 | 851，通过 | 完成 | 已注册 63 项，失败 | 未到达 | 未到达 | 流体/容器未验证 | 未验证 | 同上；需要 intrusive Fluid holder 与可执行状态定义 hook |
| Applied Energistics 2 19.2.19 + GuideME | 1825，通过 | 两者完成 | 组内已注册 37 项，失败 | 未到达 | 未到达 | 存储网络未验证 | 未验证 | `IntegerProperty.create(String,int,int)`，注册 `ae2:flawless_budding_quartz` |
| GuideME 21.1.22，独立 | 5113，通过 | 完成 | 完成，4 项 | 流程完成 | 建模 1 次 | 书本交互未验证 | 未验证 | 1 条通道元数据；真实同步与客户端未执行 |
| AppleSkin 3.0.9 | 36，通过 | 完成 | 流程完成，0 项 | 流程完成 | 建模 1 次 | 饥饿同步未验证 | HUD 未验证 | 2 条通道元数据；无玩家或真实 payload 交换 |
| FerriteCore 7.0.3 | 77，通过 | 完成，空入口 | 流程完成，0 项 | 流程完成，0 回调 | 建模 1 次 | Mixin 效果未验证 | 未验证 | 区块/状态/模型内存路径未执行 |
| Cloth Config 15.0.140 | 633，通过 | 完成，服务器分支 | 流程完成，0 项 | 流程完成，0 回调 | 建模 1 次 | 配置 GUI 未验证 | 未验证 | side 检查跳过客户端页面注册 |
| Clumps 19.0.0.1 | 15，通过 | 完成，保存 pickup 谓词 | 流程完成，0 项 | 流程完成，0 回调 | 建模 1 次 | 经验球合并未验证 | 未验证 | 实际经验球实体及 Mixin 路径未执行 |

原三模组合组 `original3` 全部解析通过；加载顺序为 **Mekanism → Ponder → YSM → Create**。Mekanism 在同一 `Block.box` 阻塞退出，后面三个尚未构造；隔离组成功不能推导组合成功。
Create 内嵌 Registrate `MC1.21-1.3.0+67` 是已扫描的库，没有独立 mods.toml 或可单独宣称完成的生命周期。Flywheel/Kuma 的客户端跳过行不计入 8 个成功组。

## 运行计数与降级说明

- 8 个成功组为 **YSM、Ponder、Curios、GuideME、AppleSkin、FerriteCore、Cloth Config、Clumps**，均记录 `unmapped=0` 与 `game_time=1`；其余 9 组退出 1。
- Curios：6 登记项、3 配置、17 通道、1 命令、21 监听器、39 事件、6 回调；GuideME 独立：4 登记项、1 通道、1 命令、2 监听器、32 事件、2 回调。
- Ponder 独立：1 登记项、1 配置、3 命令、5 监听器、32 事件、2 回调。AppleSkin：1 配置、2 通道、3 监听器、31 事件、1 回调。
- YSM：`degraded=true`、1 条 `unsatisfied_runtime_env` WARNING、0 登记项、0 通道、1 命令、15 监听器、31 事件、5 回调。JNI `System.load` 失败由模组自行捕获；退出 0 不说明原生核心、模型同步或渲染可用。
- Mekanism：0 登记项、9 配置、26 监听器，构造未完成。Create 组：12 登记项、1 配置、89 监听器，Create 构造未完成；这些计数不代表组合注册阶段完成。
- AE2 + GuideME：37 登记项、2 配置、31 监听器；Core 独立：63 登记项、2 配置、20 监听器；Core + Backpacks：74 登记项、4 配置、50 监听器。三组均未完成注册，setup/tick 未到达。

## 本轮实现与回归

- Guava Stopwatch 保留未启动/启动/停止/重置状态、累积纳秒、实际 custom ticker 调用与异常顺序；系统 ticker 使用单调时钟，不依赖现有固定值 `System.nanoTime` 桩。Duration 和格式化路径继续明确失败。
- `List.addFirst/addLast` 通过实际 `List.add` 调用保留子类 override、原对象及异常，返回 void；这两个入口对已覆盖的不可变列表拒绝修改。`Collections.addAll` 按 encounter order 调用每个 `Collection.add`，不因首次 true 而短路，保留源数组后续变化与异常前的修改。
- dust 的 **5 项回归**覆盖 REDSTONE 静态身份、共享可变红色 Vector3f 引用、构造器 scale 范围/NaN 与 null 颜色、canonical DUST 注册引用及失败边界；未实现的 dust codec、全局粒子枚举和数字 ID 明确抛错。
- Create 使用的 **11 种原版木板**保持不同 Block/单一无属性状态、准确颜色/声音/BASS/着火/强度、intrusive holder 与冻结后的相同身份；在模组构造前初始化。未伪造完整 block-state registry 或碰撞形状。
- Pulse fixture 改用真实 `BlockBehaviour.Properties` 签名，验证硬度 **2.5** 与抗爆 **7.0**；登记元数据读取实际 properties/Block 字段。补齐 ItemTags 的 STAIRS/SLABS/WALLS 与 BlockTags.WALLS，保留 item/block domain 和静态/direct factory 身份，不生成 tag 内容。
- YSM living-event subscriber 分类、Brigadier requirement/redirect/builder 快照及前序 DFU/反射/注册修复均在本次完整回归与统一 corpus 中复测；完整推进记录保留在历史页。

## 复现

在仓库根目录运行测试并构建同一检查点的探针：

```powershell
cargo test --workspace --offline
cargo build -p mmga-neoforge --profile fast --offline --example probe
cargo clippy --workspace --all-targets --offline
& target/fast/examples/probe.exe --dedicated-server '.local/mods/Mekanism-1.21.1-10.7.19.85.jar'
& target/fast/examples/probe.exe --dedicated-server '.local/mods/create-1.21.1-6.0.10.jar'
& target/fast/examples/probe.exe --dedicated-server '.local/mods/guideme-21.1.22.jar' '.local/mods/appliedenergistics2-19.2.19.jar'
```

全部固定输入和其余依赖组合的命令见[测试集复现命令](mod-corpus-2026-11-17.md#复现命令)。本机 `.local/verify-probe-corpus.py` 统一运行 17 组，要求被检查代码已提交，按 HEAD SHA 保存日志和结果，拒绝给脏代码标记提交证据。
退出 1 表示打开/准备/bootstrap/tick 失败；无参数为 2；退出 0 仅表示建模流程完成，仍应检查降级与实际功能。`--client` 只改变入口筛选，不创建窗口或渲染。

## 尚未验证的边界

MMGA 仍不能替代完整 Minecraft/NeoForge。解析、构造、登记元数据或达到 Running，都不能证明玩家协议、真实网络传输、区块/实体/流体模拟、饰品、背包、AE2 存储网络、Create 机器、JNI 或渲染正确。本轮未执行独立数据生成启动模式。
没有相同硬件、相同实际游戏负载的官方 NeoForge 对照，因此**不宣称性能提升**。历史 Linux 启动时间/RSS、空 tick 或 registry/event 微基准不能作为本轮加速证据。
