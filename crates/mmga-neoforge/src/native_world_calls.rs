//! Exact Java world calls routed to the active native callback.

use crate::{host_world::HostWorld, RegistryStore};
use mmga_classfile::MemberRef;
use mmga_runtime::{Data, Rt, V};

const SERVER: &str = "net/minecraft/server/MinecraftServer";
const LEVEL: &str = "net/minecraft/server/level/ServerLevel";
const POS: &str = "net/minecraft/core/BlockPos";
const STATE: &str = "net/minecraft/world/level/block/state/BlockState";
const CACHE: &str = "mmga/internal/NativeWorldHandles";
const SERVER_DESC: &str = "()Lnet/minecraft/server/MinecraftServer;";
const READ: &str =
    "(Lnet/minecraft/core/BlockPos;)Lnet/minecraft/world/level/block/state/BlockState;";
const SET: &str =
    "(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;I)Z";
const UPDATE: &str =
    "(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;)Z";

pub fn call(
    rt: &mut Rt,
    registries: &RegistryStore,
    world: &mut dyn HostWorld,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if rt.pending_exception.is_some() {
        return Some(V::NULL);
    }
    let is_level = matches!(
        owner,
        LEVEL
            | "net/minecraft/world/level/Level"
            | "net/minecraft/world/level/LevelAccessor"
            | "net/minecraft/world/level/LevelReader"
            | "net/minecraft/world/level/BlockGetter"
    );
    if is_static {
        if owner == "net/neoforged/neoforge/server/ServerLifecycleHooks"
            && name == "getCurrentServer"
        {
            return Some(if desc == SERVER_DESC {
                server(rt)
            } else {
                unsupported(
                    rt,
                    "Unsupported signature for ServerLifecycleHooks.getCurrentServer",
                )
            });
        }
        if is_level && name == "setBlock" {
            return Some(unsupported(
                rt,
                if owner == LEVEL && desc == "(IIILjava/lang/String;)V" {
                    "Synthetic ServerLevel.setBlock is not a native Minecraft API"
                } else {
                    "Unsupported static Level.setBlock signature"
                },
            ));
        }
        if is_level
            && matches!(
                name,
                "setBlockAndUpdate" | "getBlockState" | "getGameTime" | "getDayTime"
            )
        {
            return Some(unsupported(
                rt,
                "World query and writes are instance methods",
            ));
        }
        return None;
    }
    if is_server_event(owner) && name == "getServer" {
        if desc != SERVER_DESC {
            return Some(unsupported(
                rt,
                "Unsupported server-event getServer signature",
            ));
        }
        let Some(&receiver) = args.first() else {
            return Some(unsupported(rt, "Missing server-event receiver"));
        };
        return Some(
            if !receiver.is_null() && rt.class_of(receiver).is_some_and(is_server_event) {
                server(rt)
            } else {
                unsupported(rt, "Native server event receiver is unavailable")
            },
        );
    }
    if owner == SERVER {
        if !matches!(name, "overworld" | "getLevel") {
            return None;
        }
        if !matches!((name, desc),
            ("overworld", "()Lnet/minecraft/server/level/ServerLevel;")
            | ("getLevel", "(Lnet/minecraft/resources/ResourceKey;)Lnet/minecraft/server/level/ServerLevel;"))
        { return Some(unsupported(rt, "Unsupported MinecraftServer level-query signature")); }
        let Some(&receiver) = args.first() else {
            return Some(unsupported(rt, "Missing MinecraftServer receiver"));
        };
        if !canonical_server(rt, receiver) {
            return Some(unsupported(
                rt,
                "MinecraftServer is not the canonical native server handle",
            ));
        }
        if name == "overworld" {
            return Some(level(rt, "minecraft:overworld"));
        }
        let dimension = match args
            .get(1)
            .copied()
            .and_then(|value| dimension_key(rt, value))
        {
            Some(dimension) => dimension,
            None => return Some(unsupported(rt, "getLevel requires a dimension ResourceKey")),
        };
        return Some(match world.game_time(&dimension) {
            Ok(_) => level(rt, &dimension),
            Err(error) => unsupported(rt, &error),
        });
    }
    if !is_level {
        return None;
    }
    if !matches!(
        name,
        "setBlock" | "setBlockAndUpdate" | "getBlockState" | "getGameTime" | "getDayTime"
    ) {
        return None;
    }
    if !matches!(
        (name, desc),
        ("setBlock", SET)
            | ("setBlockAndUpdate", UPDATE)
            | ("getBlockState", READ)
            | ("getGameTime" | "getDayTime", "()J")
    ) {
        return Some(unsupported(
            rt,
            "Unsupported Minecraft Level world-call signature",
        ));
    }
    let Some(&receiver) = args.first() else {
        return Some(unsupported(rt, "Missing Level receiver"));
    };
    let Some(dimension) = canonical_dimension(rt, receiver) else {
        return Some(unsupported(
            rt,
            "Level is not a canonical native level handle",
        ));
    };
    if matches!(name, "getGameTime" | "getDayTime") {
        return Some(
            match if name == "getGameTime" {
                world.game_time(&dimension)
            } else {
                world.day_time(&dimension)
            } {
                Ok(value) => V::J(value),
                Err(error) => unsupported(rt, &error),
            },
        );
    }
    let position = match args
        .get(1)
        .copied()
        .and_then(|value| coordinates(rt, value))
    {
        Some(position) => position,
        None => {
            return Some(unsupported(
                rt,
                "Native world operation requires an initialized BlockPos",
            ))
        }
    };
    let [x, y, z] = position;
    if name == "getBlockState" {
        return Some(
            match world
                .read_block(&dimension, x, y, z)
                .and_then(|id| read_state(rt, registries, &id))
            {
                Ok(value) => value,
                Err(error) => unsupported(rt, &error),
            },
        );
    }
    let state = match args
        .get(2)
        .copied()
        .ok_or_else(|| "Missing BlockState".to_owned())
        .and_then(|state| state_name(rt, registries, state))
    {
        Ok(state) => state,
        Err(error) => return Some(unsupported(rt, &error)),
    };
    let flags = if name == "setBlockAndUpdate" {
        3
    } else {
        match args.get(3) {
            Some(V::I(flags)) => *flags,
            _ => return Some(unsupported(rt, "Invalid block update flags")),
        }
    };
    Some(match world.set_block(&dimension, x, y, z, &state, flags) {
        Ok(changed) => V::bool(changed),
        Err(error) => unsupported(rt, &error),
    })
}

fn unsupported(rt: &mut Rt, error: &str) -> V {
    rt.throw_exception("java/lang/UnsupportedOperationException", error)
}

fn is_server_event(owner: &str) -> bool {
    matches!(
        owner,
        "net/neoforged/neoforge/event/server/ServerLifecycleEvent"
            | "net/neoforged/neoforge/event/server/ServerAboutToStartEvent"
            | "net/neoforged/neoforge/event/server/ServerStartingEvent"
            | "net/neoforged/neoforge/event/server/ServerStartedEvent"
            | "net/neoforged/neoforge/event/server/ServerStoppingEvent"
            | "net/neoforged/neoforge/event/server/ServerStoppedEvent"
    )
}

fn server(rt: &mut Rt) -> V {
    if let Some(value) = rt.get_static(CACHE, "server") {
        return value;
    }
    let value = rt.new_pinned_handle(SERVER, "native_server", 0, "active");
    rt.put_static(CACHE, "server", value);
    value
}

fn canonical_server(rt: &Rt, value: V) -> bool {
    rt.class_of(value) == Some(SERVER)
        && rt.get_static(CACHE, "server") == Some(value)
        && matches!(rt.handle_of(value), Some(("native_server", 0, "active", _)))
}

fn level(rt: &mut Rt, dimension: &str) -> V {
    if let Some(value) = rt.get_static(CACHE, dimension) {
        return value;
    }
    let value = rt.new_pinned_handle(LEVEL, "native_level", 0, dimension);
    rt.put_static(CACHE, dimension, value);
    value
}

fn canonical_dimension(rt: &Rt, value: V) -> Option<String> {
    let ("native_level", 0, dimension, _) = rt.handle_of(value)? else {
        return None;
    };
    (rt.class_of(value) == Some(LEVEL) && rt.get_static(CACHE, dimension) == Some(value))
        .then(|| dimension.to_owned())
}

fn dimension_key(rt: &Rt, value: V) -> Option<String> {
    let ("resource_key", _, key, _) = rt.handle_of(value)? else {
        return None;
    };
    let (registry, dimension) = key.split_once('|')?;
    (registry == "minecraft:dimension" && resource_name(dimension)).then(|| dimension.to_owned())
}

fn resource_name(id: &str) -> bool {
    let Some((namespace, path)) = id.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && namespace
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
        && path
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-/".contains(&b))
}

fn coordinates(rt: &Rt, position: V) -> Option<[i32; 3]> {
    if rt.class_of(position) != Some(POS) {
        return None;
    }
    let [V::I(x), V::I(y), V::I(z)] = [
        rt.get_field(position, "x"),
        rt.get_field(position, "y"),
        rt.get_field(position, "z"),
    ] else {
        return None;
    };
    Some([x, y, z])
}

fn collection(rt: &Rt, value: V) -> Option<&[V]> {
    match rt.get_obj(value).map(|object| &object.data) {
        Some(Data::Array(values) | Data::List(values) | Data::Set(values)) => Some(values),
        _ => match rt
            .get_obj(rt.get_field(value, "$blockCollection"))
            .map(|object| &object.data)
        {
            Some(Data::Array(values)) => Some(values),
            _ => None,
        },
    }
}

fn state_name(rt: &Rt, registries: &RegistryStore, state: V) -> Result<String, String> {
    if rt.class_of(state) != Some(STATE) {
        return Err("Native world write requires BlockState".into());
    }
    let block = rt.get_field(state, "owner");
    let entries = registries
        .entries
        .get("minecraft:block")
        .ok_or("Block registry is unavailable")?;
    let mut matches = entries
        .values()
        .filter(|entry| !block.is_null() && entry.value == block);
    let entry = matches
        .next()
        .ok_or("BlockState owner has no canonical registered block")?;
    if matches.next().is_some() || !resource_name(&entry.id) {
        return Err("BlockState registry identity is ambiguous".into());
    }
    let definition = rt.get_field(block, "stateDefinition");
    if definition.is_null()
        || rt.get_field(state, "$definition") != definition
        || !collection(rt, rt.get_field(definition, "states"))
            .is_some_and(|states| states.contains(&state))
    {
        return Err("BlockState is outside its registered owner's canonical state graph".into());
    }
    match rt
        .get_obj(rt.get_field(state, "$stateValues"))
        .map(|object| &object.data)
    {
        Some(Data::Map(values)) if values.is_empty() => Ok(entry.id.clone()),
        _ => Err("Native serialization of block properties is not implemented".into()),
    }
}

fn read_state(rt: &Rt, registries: &RegistryStore, name: &str) -> Result<V, String> {
    if !resource_name(name) {
        return Err("Native state has properties or an invalid identifier".into());
    }
    let entry = registries
        .lookup("minecraft:block", name)
        .ok_or("Native block has no modeled definition")?;
    let state = rt.get_field(entry.value, "defaultBlockState");
    if state_name(rt, registries, state)? != name {
        return Err("Native block has no propertyless canonical state".into());
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, ExecCtx, HostApi, LinkedSpace};

    #[derive(Debug, Default)]
    struct World {
        writes: Vec<(String, [i32; 3], String, i32)>,
        state: String,
        fail: bool,
        changed: bool,
    }
    impl HostWorld for World {
        fn read_block(
            &mut self,
            dimension: &str,
            x: i32,
            y: i32,
            z: i32,
        ) -> Result<String, String> {
            assert_eq!(
                (dimension, [x, y, z]),
                ("minecraft:overworld", [-2, 75, 19])
            );
            if self.fail {
                Err("native read failed".into())
            } else {
                Ok(self.state.clone())
            }
        }
        fn set_block(
            &mut self,
            dimension: &str,
            x: i32,
            y: i32,
            z: i32,
            state: &str,
            flags: i32,
        ) -> Result<bool, String> {
            self.writes
                .push((dimension.into(), [x, y, z], state.into(), flags));
            if self.fail {
                Err("native write refused".into())
            } else {
                Ok(self.changed)
            }
        }
        fn game_time(&mut self, dimension: &str) -> Result<i64, String> {
            if self.fail {
                return Err("native time unavailable".into());
            }
            match dimension {
                "minecraft:overworld" => Ok(i64::MAX - 3),
                "test:dimension" => Ok(400),
                _ => Err("dimension unavailable".into()),
            }
        }
        fn day_time(&mut self, _: &str) -> Result<i64, String> {
            if self.fail {
                Err("native time unavailable".into())
            } else {
                Ok(-12)
            }
        }
    }

    struct Host {
        registries: RegistryStore,
        world: World,
    }
    impl HostApi for Host {
        fn get_static_field(&mut self, _: &mut Rt, _: &str, _: &str, _: &str) -> Option<V> {
            None
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            _: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            call(
                rt,
                &self.registries,
                &mut self.world,
                member,
                is_static,
                args,
            )
        }
    }

    fn invoke(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            if name == "getCurrentServer" {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(owner, name, desc),
            args,
        )
    }

    fn fixture() -> (Rt, Host, V, V, V) {
        let mut rt = Rt::new();
        let mut registries = RegistryStore::new();
        crate::blocks::bootstrap_planks(&mut rt, &mut registries);
        assert!(rt.pending_exception.is_none());
        let block = registries
            .lookup("minecraft:block", "minecraft:oak_planks")
            .unwrap()
            .value;
        let state = rt.get_field(block, "defaultBlockState");
        let pos = rt.new_instance(POS, 0);
        for (field, value) in [("x", -2), ("y", 75), ("z", 19)] {
            rt.put_field(pos, field, V::I(value));
        }
        let server = server(&mut rt);
        let level = level(&mut rt, "minecraft:overworld");
        assert!(canonical_server(&rt, server));
        (
            rt,
            Host {
                registries,
                world: World {
                    state: "minecraft:oak_planks".into(),
                    ..World::default()
                },
            },
            level,
            pos,
            state,
        )
    }

    #[test]
    fn native_calls_preserve_canonical_handles_actual_state_arguments_false_and_time() {
        let (mut rt, mut host, level, pos, state) = fixture();
        let linked = LinkedSpace::new();
        let event = rt.new_instance("net/neoforged/neoforge/event/server/ServerStartedEvent", 0);
        let dim =
            crate::minecraft::new_resource_key(&mut rt, "minecraft:dimension", "test:dimension");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let server = invoke(
            &mut ctx,
            "net/neoforged/neoforge/server/ServerLifecycleHooks",
            "getCurrentServer",
            SERVER_DESC,
            &[],
        );
        assert_eq!(
            server,
            invoke(
                &mut ctx,
                "net/neoforged/neoforge/event/server/ServerLifecycleEvent",
                "getServer",
                SERVER_DESC,
                &[event]
            )
        );
        assert_eq!(
            level,
            invoke(
                &mut ctx,
                SERVER,
                "overworld",
                "()Lnet/minecraft/server/level/ServerLevel;",
                &[server]
            )
        );
        let other = invoke(
            &mut ctx,
            SERVER,
            "getLevel",
            "(Lnet/minecraft/resources/ResourceKey;)Lnet/minecraft/server/level/ServerLevel;",
            &[server, dim],
        );
        assert_eq!(
            canonical_dimension(ctx.rt, other).as_deref(),
            Some("test:dimension")
        );
        assert_eq!(
            state,
            invoke(&mut ctx, LEVEL, "getBlockState", READ, &[level, pos])
        );
        assert_eq!(
            invoke(&mut ctx, LEVEL, "getGameTime", "()J", &[level]),
            V::J(i64::MAX - 3)
        );
        assert_eq!(
            invoke(&mut ctx, LEVEL, "getDayTime", "()J", &[level]),
            V::J(-12)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                LEVEL,
                "setBlock",
                SET,
                &[level, pos, state, V::I(18)]
            ),
            V::FALSE
        );
        assert_eq!(
            invoke(
                &mut ctx,
                LEVEL,
                "setBlockAndUpdate",
                UPDATE,
                &[level, pos, state]
            ),
            V::FALSE
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            host.world.writes,
            vec![
                (
                    "minecraft:overworld".into(),
                    [-2, 75, 19],
                    "minecraft:oak_planks".into(),
                    18
                ),
                (
                    "minecraft:overworld".into(),
                    [-2, 75, 19],
                    "minecraft:oak_planks".into(),
                    3
                ),
            ]
        );
    }

    #[test]
    fn native_failures_unmodeled_states_and_forged_receivers_cannot_report_success() {
        let (mut rt, mut host, level, pos, state) = fixture();
        let fake_level = rt.new_pinned_handle(LEVEL, "native_level", 0, "minecraft:overworld");
        let blank_pos = rt.new_instance(POS, 0);
        let forged = rt.new_instance(STATE, 0);
        let block = rt.get_field(state, "owner");
        let definition = rt.get_field(state, "$definition");
        rt.put_field(forged, "owner", block);
        rt.put_field(forged, "$definition", definition);
        let properties = rt.alloc("java/util/Map", 0, Data::Map(Vec::new()));
        rt.put_field(forged, "$stateValues", properties);
        for arguments in [
            vec![fake_level, pos, state],
            vec![level, blank_pos, state],
            vec![level, pos, forged],
        ] {
            assert_eq!(
                call(
                    &mut rt,
                    &host.registries,
                    &mut host.world,
                    &MemberRef::new(LEVEL, "setBlockAndUpdate", UPDATE),
                    false,
                    &arguments
                ),
                Some(V::NULL)
            );
            let error = rt.take_exception().unwrap();
            assert_eq!(
                rt.class_of(error),
                Some("java/lang/UnsupportedOperationException")
            );
        }
        assert!(
            host.world.writes.is_empty(),
            "validation failures must never reach the host"
        );
        host.world.fail = true;
        assert_eq!(
            call(
                &mut rt,
                &host.registries,
                &mut host.world,
                &MemberRef::new(LEVEL, "setBlockAndUpdate", UPDATE),
                false,
                &[level, pos, state]
            ),
            Some(V::NULL)
        );
        let error = rt.take_exception().unwrap();
        assert!(rt.exception_summary(error).contains("native write refused"));
        assert_eq!(host.world.writes.len(), 1);
        host.world.fail = false;
        host.world.changed = true;
        assert_eq!(
            call(
                &mut rt,
                &host.registries,
                &mut host.world,
                &MemberRef::new(LEVEL, "setBlockAndUpdate", UPDATE),
                false,
                &[level, pos, state]
            ),
            Some(V::TRUE)
        );
        assert!(rt.pending_exception.is_none());
        let before = host.world.writes.len();
        assert_eq!(
            call(
                &mut rt,
                &host.registries,
                &mut host.world,
                &MemberRef::new(LEVEL, "setBlock", "(IIILjava/lang/String;)V"),
                true,
                &[V::I(0), V::I(0), V::I(0), V::NULL]
            ),
            Some(V::NULL)
        );
        let error = rt.take_exception().unwrap();
        assert!(rt.exception_summary(error).contains("Synthetic"));
        assert_eq!(host.world.writes.len(), before);
        assert_eq!(
            call(
                &mut rt,
                &host.registries,
                &mut host.world,
                &MemberRef::new(LEVEL, "setBlock", "()Z"),
                false,
                &[level]
            ),
            Some(V::NULL)
        );
        let error = rt.take_exception().unwrap();
        assert!(rt.exception_summary(error).contains("signature"));
        for text in [
            "minecraft:oak_planks[waterlogged=false]",
            "minecraft:stone",
            "",
            "BAD:block",
        ] {
            host.world.state = text.into();
            assert_eq!(
                call(
                    &mut rt,
                    &host.registries,
                    &mut host.world,
                    &MemberRef::new(LEVEL, "getBlockState", READ),
                    false,
                    &[level, pos]
                ),
                Some(V::NULL)
            );
            let error = rt.take_exception().unwrap();
            assert_eq!(
                rt.class_of(error),
                Some("java/lang/UnsupportedOperationException")
            );
        }
    }
}
