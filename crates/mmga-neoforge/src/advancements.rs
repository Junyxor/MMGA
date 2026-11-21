//! SimpleCriterionTrigger's listener state from the official 1.21.1 `dv` class.
//! The player map uses identity keys, while each listener set uses Java
//! hashCode/equals. Actual advancement triggering still needs a real player and
//! LootContext; those calls remain unsupported rather than awarding fake progress.
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, Data, ExecCtx, V};

const SIMPLE: &str = "net/minecraft/advancements/critereon/SimpleCriterionTrigger";
const TRIGGER: &str = "net/minecraft/advancements/CriterionTrigger";

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static {
        return None;
    }
    let recv = *args.first()?;
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if member.owner == SIMPLE && name == "<init>" && desc == "()V" {
        let players = ctx
            .rt
            .alloc("java/util/IdentityHashMap", 0, Data::Map(Vec::new()));
        ctx.rt.put_field(recv, "players", players);
        return Some(V::Void);
    }
    let class = ctx.rt.class_of(recv)?;
    if class != SIMPLE && !ctx.linked.is_subtype(class, SIMPLE, ctx.host) {
        return None;
    }
    if member.owner != SIMPLE
        && member.owner != TRIGGER
        && !ctx.linked.is_subtype(&member.owner, SIMPLE, ctx.host)
    {
        return None;
    }
    let player = *args.get(1)?;
    let map = ctx.rt.get_field(recv, "players");
    let mut players = match ctx.rt.get_obj(map).map(|v| &v.data) {
        Some(Data::Map(players)) => players.clone(),
        _ => return None,
    };
    let index = players.iter().position(|(key, _)| *key == player);
    if name == "removePlayerListeners" && desc == "(Lnet/minecraft/server/PlayerAdvancements;)V" {
        if let Some(index) = index {
            players.remove(index);
            ctx.rt.get_obj_mut(map)?.data = Data::Map(players);
        }
        return Some(V::Void);
    }
    if !matches!(name, "addPlayerListener" | "removePlayerListener")
        || desc != "(Lnet/minecraft/server/PlayerAdvancements;Lnet/minecraft/advancements/CriterionTrigger$Listener;)V"
    {
        return None;
    }
    let add = name == "addPlayerListener";
    let listener = *args.get(2)?;
    let index = match index {
        Some(index) => index,
        None if !add => return Some(V::Void),
        None => {
            let set = ctx.rt.alloc("java/util/HashSet", 0, Data::Set(Vec::new()));
            let hashes = ctx.rt.alloc("[I", 0, Data::Array(Vec::new()));
            ctx.rt
                .put_field(recv, format!("$listener_hashes:{}", set.r()), hashes);
            players.push((player, set));
            // computeIfAbsent commits the new bucket before HashSet.add runs.
            ctx.rt.get_obj_mut(map)?.data = Data::Map(players.clone());
            players.len() - 1
        }
    };
    let set = players[index].1;
    let hashes = ctx
        .rt
        .get_field(recv, &format!("$listener_hashes:{}", set.r()));
    let mut values = match ctx.rt.get_obj(set).map(|v| &v.data) {
        Some(Data::Set(values)) => values.clone(),
        _ => return None,
    };
    let mut stored_hashes = match ctx.rt.get_obj(hashes).map(|v| &v.data) {
        Some(Data::Array(values)) => values.clone(),
        _ => return None,
    };
    if stored_hashes.len() != values.len() {
        return None;
    }
    // HashMap.remove returns before hashing the key when the bucket map is empty.
    if values.is_empty() && !add {
        players.remove(index);
        ctx.rt.get_obj_mut(map)?.data = Data::Map(players);
        return Some(V::Void);
    }
    let hash = if listener.is_null() {
        V::I(0)
    } else {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new("java/lang/Object", "hashCode", "()I"),
            &[listener],
        )
    };
    if ctx.rt.pending_exception.is_some() {
        return Some(V::NULL);
    }
    let mut existing = None;
    for (position, value) in values.iter().enumerate() {
        if stored_hashes[position] != hash {
            continue;
        }
        let equal = if *value == listener {
            true
        } else if listener.is_null() {
            false
        } else {
            let tested = invoke_member(
                ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new("java/lang/Object", "equals", "(Ljava/lang/Object;)Z"),
                &[listener, *value],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            tested == V::TRUE
        };
        if equal {
            existing = Some(position);
            break;
        }
    }
    match (add, existing) {
        (true, None) => {
            values.push(listener);
            stored_hashes.push(hash);
        }
        (false, Some(position)) => {
            values.remove(position);
            stored_hashes.remove(position);
        }
        _ => {}
    }
    let empty = values.is_empty();
    ctx.rt.get_obj_mut(set)?.data = Data::Set(values);
    ctx.rt.get_obj_mut(hashes)?.data = Data::Array(stored_hashes);
    if !add && empty {
        players.remove(index);
        ctx.rt.get_obj_mut(map)?.data = Data::Map(players);
    }
    Some(V::Void)
}

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    sub == SIMPLE && sup == TRIGGER
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{HostApi, LinkedSpace, NativeClassDef, NativeModuleRegistry, Rt};

    struct AdvancementHost;
    impl HostApi for AdvancementHost {
        fn get_static_field(
            &mut self,
            _rt: &mut Rt,
            _owner: &str,
            _name: &str,
            _desc: &str,
        ) -> Option<V> {
            None
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(
                &mut ExecCtx {
                    rt,
                    linked,
                    host: self,
                    depth: 0,
                },
                member,
                is_static,
                args,
            )
        }
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            super::is_subtype(sub, sup)
        }
    }

    fn hash(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        if ctx.rt.get_field(args[0], "throwHash") == V::TRUE {
            return ctx.rt.throw_exception("test/HashFailure", "listener hash");
        }
        ctx.rt.get_field(args[0], "key")
    }

    fn equals(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let count = ctx.rt.get_field(args[0], "equalsCalls").i() + 1;
        ctx.rt.put_field(args[0], "equalsCalls", V::I(count));
        V::bool(ctx.rt.get_field(args[0], "key") == ctx.rt.get_field(args[1], "key"))
    }

    fn linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[
                NativeClassDef {
                    name: "test/Trigger",
                    superclass: Some(SIMPLE),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[],
                },
                NativeClassDef {
                    name: "test/EqualObject",
                    superclass: Some("java/lang/Object"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[
                        ("hashCode.()I", hash, false, false),
                        ("equals.(Ljava/lang/Object;)Z", equals, false, false),
                    ],
                },
            ],
            lambdas: &[],
        });
        linked
    }

    fn invoke(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(owner, name, desc),
            args,
        )
    }

    #[test]
    fn trigger_constructor_preserves_subclass_fields_and_owns_a_fresh_identity_player_map() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = AdvancementHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let first = ctx.rt.new_instance("test/Trigger", 0);
        ctx.rt.put_field(first, "subclassState", V::I(42));
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(SIMPLE, "<init>", "()V"),
            &[first],
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "{:?}",
            ctx.rt.pending_exception
        );
        assert_eq!(ctx.rt.get_field(first, "subclassState"), V::I(42));
        let map = ctx.rt.get_field(first, "players");
        assert_eq!(ctx.rt.class_of(map), Some("java/util/IdentityHashMap"));
        assert!(
            matches!(ctx.rt.get_obj(map).map(|v| &v.data), Some(Data::Map(entries)) if entries.is_empty())
        );
        let second = ctx.rt.new_instance("test/Trigger", 0);
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(SIMPLE, "<init>", "()V"),
            &[second],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_ne!(ctx.rt.get_field(second, "players"), map);
    }

    #[test]
    fn equal_players_stay_separate_while_listener_sets_call_real_hash_and_equals() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = AdvancementHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let trigger = ctx.rt.new_instance("test/Trigger", 0);
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(SIMPLE, "<init>", "()V"),
            &[trigger],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let a = ctx.rt.new_instance("test/EqualObject", 0);
        let b = ctx.rt.new_instance("test/EqualObject", 0);
        ctx.rt.put_field(a, "key", V::I(10));
        ctx.rt.put_field(b, "key", V::I(10));
        let listener = ctx.rt.new_instance("test/EqualObject", 0);
        let alias = ctx.rt.new_instance("test/EqualObject", 0);
        ctx.rt.put_field(listener, "key", V::I(20));
        ctx.rt.put_field(alias, "key", V::I(20));
        let desc = "(Lnet/minecraft/server/PlayerAdvancements;Lnet/minecraft/advancements/CriterionTrigger$Listener;)V";
        for (player, listener) in [(a, listener), (a, alias), (b, listener)] {
            invoke(
                &mut ctx,
                TRIGGER,
                "addPlayerListener",
                desc,
                &[trigger, player, listener],
            );
            assert!(
                ctx.rt.pending_exception.is_none(),
                "{}",
                ctx.rt
                    .pending_exception
                    .map(|id| ctx.rt.exception_summary(V::R(id)))
                    .unwrap_or_default()
            );
        }
        let map = ctx.rt.get_field(trigger, "players");
        let entries = match &ctx.rt.get_obj(map).unwrap().data {
            Data::Map(v) => v.clone(),
            _ => panic!("not a map"),
        };
        assert_eq!(entries.len(), 2);
        for (_, set) in &entries {
            assert!(
                matches!(ctx.rt.get_obj(*set).map(|v| &v.data), Some(Data::Set(values)) if values.len() == 1)
            );
        }
        assert_eq!(ctx.rt.get_field(alias, "equalsCalls"), V::I(1));
        assert_eq!(ctx.rt.get_field(b, "equalsCalls"), V::NULL);
        invoke(
            &mut ctx,
            TRIGGER,
            "removePlayerListener",
            desc,
            &[trigger, a, alias],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let entries = match &ctx.rt.get_obj(map).unwrap().data {
            Data::Map(v) => v.clone(),
            _ => panic!("not a map"),
        };
        assert_eq!(entries.iter().map(|(p, _)| *p).collect::<Vec<_>>(), vec![b]);
        invoke(
            &mut ctx,
            TRIGGER,
            "removePlayerListeners",
            "(Lnet/minecraft/server/PlayerAdvancements;)V",
            &[trigger, b],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert!(matches!(ctx.rt.get_obj(map).map(|v| &v.data), Some(Data::Map(v)) if v.is_empty()));
    }

    #[test]
    fn failing_listener_hash_keeps_the_created_empty_bucket_and_world_trigger_stays_unsupported() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = AdvancementHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let trigger = ctx.rt.new_instance("test/Trigger", 0);
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(SIMPLE, "<init>", "()V"),
            &[trigger],
        );
        let listener = ctx.rt.new_instance("test/EqualObject", 0);
        ctx.rt.put_field(listener, "throwHash", V::TRUE);
        invoke(&mut ctx, TRIGGER, "addPlayerListener", "(Lnet/minecraft/server/PlayerAdvancements;Lnet/minecraft/advancements/CriterionTrigger$Listener;)V", &[trigger, V::NULL, listener]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(
            ctx.rt.exception_summary(error).contains("test.HashFailure"),
            "{}",
            ctx.rt.exception_summary(error)
        );
        let map = ctx.rt.get_field(trigger, "players");
        let entries = match &ctx.rt.get_obj(map).unwrap().data {
            Data::Map(v) => v.clone(),
            _ => panic!("not a map"),
        };
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, V::NULL);
        assert!(
            matches!(ctx.rt.get_obj(entries[0].1).map(|v| &v.data), Some(Data::Set(v)) if v.is_empty())
        );
        invoke(
            &mut ctx,
            SIMPLE,
            "trigger",
            "(Lnet/minecraft/server/level/ServerPlayer;Ljava/util/function/Predicate;)V",
            &[trigger, V::NULL, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
}
