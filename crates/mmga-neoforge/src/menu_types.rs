//! NeoForge MenuType factory preservation and Minecraft vanilla feature flags.
//!
//! Official NeoForge 1.21.1 IMenuTypeExtension/IContainerFactory + MenuType patch;
//! Minecraft 1.21.1 `FeatureFlags` (`cpn`) sets DEFAULT_FLAGS = VANILLA_SET.
//! Constructing a menu type does not create a menu, open a screen, or simulate it.

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, Data, ExecCtx, HostApi, LinkedSpace, Rt, V};

const MENU: &str = "net/minecraft/world/inventory/MenuType";
const EXTENSION: &str = "net/neoforged/neoforge/common/extensions/IMenuTypeExtension";
const FACTORY: &str = "net/neoforged/neoforge/network/IContainerFactory";
const SUPPLIER: &str = "net/minecraft/world/inventory/MenuType$MenuSupplier";
const FLAGS: &str = "net/minecraft/world/flag/FeatureFlags";
const FLAG: &str = "net/minecraft/world/flag/FeatureFlag";
const FLAG_SET: &str = "net/minecraft/world/flag/FeatureFlagSet";
const UNIVERSE: &str = "net/minecraft/world/flag/FeatureFlagUniverse";
const CREATE2: &str = "(ILnet/minecraft/world/entity/player/Inventory;)Lnet/minecraft/world/inventory/AbstractContainerMenu;";
const CREATE3: &str = "(ILnet/minecraft/world/entity/player/Inventory;Lnet/minecraft/network/RegistryFriendlyByteBuf;)Lnet/minecraft/world/inventory/AbstractContainerMenu;";

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != FLAGS {
        return None;
    }
    if desc == "Lnet/minecraft/world/flag/FeatureFlag;" {
        let index = match name {
            "VANILLA" => 0,
            "BUNDLE" => 1,
            "TRADE_REBALANCE" => 2,
            _ => return None,
        };
        if let Some(flag) = rt.get_static(owner, name) {
            return Some(flag);
        }
        let universe = main_universe(rt);
        let flag = rt.new_instance(FLAG, 0);
        rt.put_field(flag, "universe", universe);
        rt.put_field(flag, "mask", V::J(1i64 << index));
        rt.put_static(owner, name, flag);
        return Some(flag);
    }
    if desc == "Lnet/minecraft/world/flag/FeatureFlagSet;"
        && matches!(name, "VANILLA_SET" | "DEFAULT_FLAGS")
    {
        if let Some(flags) = rt.get_static(FLAGS, "VANILLA_SET") {
            return Some(flags);
        }
        let universe = main_universe(rt);
        let flags = flag_set(rt, universe, 1);
        rt.put_static(FLAGS, "VANILLA_SET", flags);
        rt.put_static(FLAGS, "DEFAULT_FLAGS", flags);
        return Some(flags);
    }
    None
}
pub fn call(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if !is_static && matches!(owner, FACTORY | SUPPLIER) && name == "create" && desc == CREATE2 {
        let recv = *args.first()?;
        if linked.is_subtype(rt.class_of(recv).unwrap_or(""), FACTORY, host)
            || matches!(rt.get_obj(recv).map(|o|&o.data),Some(Data::Closure(closure)) if closure.interface_class==FACTORY)
        {
            let mut ctx = ExecCtx {
                rt,
                linked,
                host,
                depth: 0,
            };
            return Some(invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new(FACTORY, "create", CREATE3),
                &[recv, *args.get(1)?, *args.get(2)?, V::NULL],
            ));
        }
    }
    if owner == EXTENSION && is_static && name == "create" && desc == "(Lnet/neoforged/neoforge/network/IContainerFactory;)Lnet/minecraft/world/inventory/MenuType;" {
        let factory=*args.first()?;
        let flags=get_static_field(rt,FLAGS,"DEFAULT_FLAGS","Lnet/minecraft/world/flag/FeatureFlagSet;")?;
        let menu=rt.new_instance(MENU,0);rt.put_field(menu,"constructor",factory);rt.put_field(menu,"requiredFeatures",flags);return Some(menu);
    }
    if !is_static && matches!(owner, MENU | EXTENSION) {
        let recv = *args.first()?;
        if owner==MENU && name=="<init>" && desc=="(Lnet/minecraft/world/inventory/MenuType$MenuSupplier;Lnet/minecraft/world/flag/FeatureFlagSet;)V" {
            rt.put_field(recv,"constructor",*args.get(1)?);rt.put_field(recv,"requiredFeatures",*args.get(2)?);return Some(V::Void);
        }
        if name == "requiredFeatures" && desc == "()Lnet/minecraft/world/flag/FeatureFlagSet;" {
            return Some(rt.get_field(recv, "requiredFeatures"));
        }
        if name=="create"&&(desc==CREATE2||desc==CREATE3||desc=="(ILnet/minecraft/world/entity/player/Inventory;Lnet/minecraft/network/RegistryFriendlyByteBuf;)Ljava/lang/Object;") {
            let factory=rt.get_field(recv,"constructor");
            let extended=linked.is_subtype(rt.class_of(factory).unwrap_or(""),FACTORY,host)
                || matches!(rt.get_obj(factory).map(|o|&o.data),Some(Data::Closure(closure)) if closure.interface_class==FACTORY);
            let window=*args.get(1)?;let inventory=*args.get(2)?;
            let mut ctx=ExecCtx{rt,linked,host,depth:0};
            return Some(if extended {
                let extra=args.get(3).copied().unwrap_or(V::NULL);
                invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new(FACTORY,"create",CREATE3),&[factory,window,inventory,extra])
            } else {
                invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new(SUPPLIER,"create",CREATE2),&[factory,window,inventory])
            });
        }
    }
    if owner == FLAG_SET {
        if is_static && name == "of" {
            return match desc {
                "()Lnet/minecraft/world/flag/FeatureFlagSet;"=>Some(empty_flags(rt)),
                "(Lnet/minecraft/world/flag/FeatureFlag;)Lnet/minecraft/world/flag/FeatureFlagSet;"=>{
                    let flag=*args.first()?;
                    if flag.is_null(){return Some(rt.throw_exception("java/lang/NullPointerException","FeatureFlagSet.of(null)"));}
                    Some(flag_set(rt,rt.get_field(flag,"universe"),rt.get_field(flag,"mask").j()))
                }
                _=>None,
            };
        }
        if !is_static {
            let recv = *args.first()?;
            return match (name, desc) {
                ("contains", "(Lnet/minecraft/world/flag/FeatureFlag;)Z") => {
                    let flag = *args.get(1)?;
                    if flag.is_null() {
                        return Some(rt.throw_exception(
                            "java/lang/NullPointerException",
                            "FeatureFlagSet.contains(null)",
                        ));
                    }
                    Some(V::bool(
                        rt.get_field(recv, "universe") == rt.get_field(flag, "universe")
                            && (rt.get_field(recv, "mask").j() & rt.get_field(flag, "mask").j())
                                != 0,
                    ))
                }
                ("isEmpty", "()Z") => Some(V::bool(
                    rt.get_field(recv, "universe").is_null() && rt.get_field(recv, "mask").j() == 0,
                )),
                ("isSubsetOf", "(Lnet/minecraft/world/flag/FeatureFlagSet;)Z") => {
                    if rt.get_field(recv, "universe").is_null() {
                        return Some(V::TRUE);
                    }
                    let other = *args.get(1)?;
                    if other.is_null() {
                        return Some(rt.throw_exception(
                            "java/lang/NullPointerException",
                            "FeatureFlagSet.isSubsetOf(null)",
                        ));
                    }
                    Some(V::bool(
                        rt.get_field(recv, "universe") == rt.get_field(other, "universe")
                            && (rt.get_field(recv, "mask").j() & !rt.get_field(other, "mask").j())
                                == 0,
                    ))
                }
                ("equals", "(Ljava/lang/Object;)Z") => {
                    let other = *args.get(1)?;
                    Some(V::bool(
                        rt.class_of(other) == Some(FLAG_SET)
                            && rt.get_field(recv, "universe") == rt.get_field(other, "universe")
                            && rt.get_field(recv, "mask") == rt.get_field(other, "mask"),
                    ))
                }
                ("hashCode", "()I") => {
                    // FeatureFlagSet delegates to fastutil HashCommon.mix(long).
                    let mut hash =
                        (rt.get_field(recv, "mask").j() as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
                    hash ^= hash >> 32;
                    Some(V::I((hash ^ (hash >> 16)) as i32))
                }
                _ => None,
            };
        }
    }
    if owner == FLAGS
        && is_static
        && name == "isExperimental"
        && desc == "(Lnet/minecraft/world/flag/FeatureFlagSet;)Z"
    {
        let flags = *args.first()?;
        if flags.is_null() {
            return Some(rt.throw_exception(
                "java/lang/NullPointerException",
                "FeatureFlags.isExperimental(null)",
            ));
        }
        let vanilla = get_static_field(
            rt,
            FLAGS,
            "VANILLA_SET",
            "Lnet/minecraft/world/flag/FeatureFlagSet;",
        )?;
        let subset = rt.get_field(flags, "universe").is_null()
            || (rt.get_field(flags, "universe") == rt.get_field(vanilla, "universe")
                && (rt.get_field(flags, "mask").j() & !1) == 0);
        return Some(V::bool(!subset));
    }
    None
}

fn main_universe(rt: &mut Rt) -> V {
    if let Some(universe) = rt.get_static(FLAGS, "$main_universe") {
        return universe;
    }
    let universe = rt.new_instance(UNIVERSE, 0);
    let id = rt.intern_str("main");
    rt.put_field(universe, "id", id);
    rt.put_static(FLAGS, "$main_universe", universe);
    universe
}
fn flag_set(rt: &mut Rt, universe: V, mask: i64) -> V {
    let set = rt.new_instance(FLAG_SET, 0);
    rt.put_field(set, "universe", universe);
    rt.put_field(set, "mask", V::J(mask));
    set
}
fn empty_flags(rt: &mut Rt) -> V {
    if let Some(empty) = rt.get_static(FLAG_SET, "EMPTY") {
        return empty;
    }
    let empty = flag_set(rt, V::NULL, 0);
    rt.put_static(FLAG_SET, "EMPTY", empty);
    empty
}
pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    (sub_class == FACTORY && super_class == SUPPLIER)
        || (sub_class == MENU
            && matches!(
                super_class,
                EXTENSION | "net/minecraft/world/flag/FeatureElement"
            ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{
        invoke_member, read_static_field, ClosureData, ClosureTarget, Data, ExecCtx,
        NativeLambdaFn, NativeModuleRegistry,
    };
    struct Host;
    impl HostApi for Host {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            super::get_static_field(rt, owner, name, desc)
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, linked, self, member, is_static, args)
        }
        fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
            super::is_subtype(sub_class, super_class)
        }
    }
    fn create(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let calls = ctx.rt.get_field(args[0], "calls").i();
        ctx.rt.put_field(args[0], "calls", V::I(calls + 1));
        let result = ctx.rt.new_instance("example/ActualMenu", 0);
        ctx.rt.put_field(result, "windowId", args[1]);
        ctx.rt.put_field(result, "inventory", args[2]);
        ctx.rt
            .put_field(result, "argumentCount", V::I((args.len() - 1) as i32));
        ctx.rt
            .put_field(result, "extraData", args.get(3).copied().unwrap_or(V::NULL));
        result
    }
    fn fail(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
        ctx.rt
            .throw_exception("example/MenuFailure", "real factory failure")
    }
    static LAMBDAS: &[(u32, NativeLambdaFn)] = &[(1, create), (2, fail)];
    fn linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[],
            lambdas: LAMBDAS,
        });
        linked
    }
    fn factory(rt: &mut Rt, counter: V, extended: bool, id: u32) -> V {
        let interface = if extended { FACTORY } else { SUPPLIER };
        let desc = if extended {
            "(ILnet/minecraft/world/entity/player/Inventory;Lnet/minecraft/network/RegistryFriendlyByteBuf;)Lnet/minecraft/world/inventory/AbstractContainerMenu;"
        } else {
            "(ILnet/minecraft/world/entity/player/Inventory;)Lnet/minecraft/world/inventory/AbstractContainerMenu;"
        };
        rt.alloc(
            interface,
            0,
            Data::Closure(Box::new(ClosureData {
                interface_class: interface.into(),
                sam_name: "create".into(),
                sam_desc: desc.into(),
                instantiated_desc: desc.into(),
                captures: vec![counter],
                target: ClosureTarget::NativeLambda(id),
            })),
        )
    }
    fn invoke(
        ctx: &mut ExecCtx<'_>,
        owner: &str,
        name: &str,
        desc: &str,
        is_static: bool,
        args: &[V],
    ) -> V {
        invoke_member(
            ctx,
            if is_static {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    #[test]
    fn extension_preserves_factory_and_passes_actual_window_inventory_and_payload() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let counter = rt.new_instance("example/Counter", 0);
        let factory = factory(&mut rt, counter, true, 1);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let menu=invoke(&mut ctx,EXTENSION,"create","(Lnet/neoforged/neoforge/network/IContainerFactory;)Lnet/minecraft/world/inventory/MenuType;",true,&[factory]);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(counter, "calls").i(), 0);
        let flags = invoke(
            &mut ctx,
            MENU,
            "requiredFeatures",
            "()Lnet/minecraft/world/flag/FeatureFlagSet;",
            false,
            &[menu],
        );
        assert_eq!(
            flags,
            read_static_field(
                &mut ctx,
                FLAGS,
                "DEFAULT_FLAGS",
                "Lnet/minecraft/world/flag/FeatureFlagSet;"
            )
        );
        let vanilla = read_static_field(
            &mut ctx,
            FLAGS,
            "VANILLA",
            "Lnet/minecraft/world/flag/FeatureFlag;",
        );
        let bundle = read_static_field(
            &mut ctx,
            FLAGS,
            "BUNDLE",
            "Lnet/minecraft/world/flag/FeatureFlag;",
        );
        assert_eq!(
            invoke(
                &mut ctx,
                FLAG_SET,
                "contains",
                "(Lnet/minecraft/world/flag/FeatureFlag;)Z",
                false,
                &[flags, vanilla]
            ),
            V::TRUE
        );
        assert_eq!(
            invoke(
                &mut ctx,
                FLAG_SET,
                "contains",
                "(Lnet/minecraft/world/flag/FeatureFlag;)Z",
                false,
                &[flags, bundle]
            ),
            V::FALSE
        );
        let inventory = ctx
            .rt
            .new_instance("net/minecraft/world/entity/player/Inventory", 0);
        let bytes = ctx.rt.alloc(
            "net/minecraft/network/RegistryFriendlyByteBuf",
            0,
            Data::ByteBuf {
                bytes: vec![1, 2, 3],
                reader_idx: 0,
            },
        );
        let instance=invoke(&mut ctx,MENU,"create","(ILnet/minecraft/world/entity/player/Inventory;Lnet/minecraft/network/RegistryFriendlyByteBuf;)Lnet/minecraft/world/inventory/AbstractContainerMenu;",false,&[menu,V::I(12),inventory,bytes]);
        assert_eq!(ctx.rt.get_field(instance, "windowId"), V::I(12));
        assert_eq!(ctx.rt.get_field(instance, "inventory"), inventory);
        assert_eq!(ctx.rt.get_field(instance, "extraData"), bytes);
        let no_data=invoke(&mut ctx,MENU,"create","(ILnet/minecraft/world/entity/player/Inventory;)Lnet/minecraft/world/inventory/AbstractContainerMenu;",false,&[menu,V::I(13),inventory]);
        assert_eq!(ctx.rt.get_field(no_data, "extraData"), V::NULL);
        assert_eq!(ctx.rt.get_field(counter, "calls"), V::I(2));
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn vanilla_supplier_fallback_and_exceptions_are_preserved() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let counter = rt.new_instance("example/Counter", 0);
        let plain = factory(&mut rt, counter, false, 1);
        let failure = factory(&mut rt, counter, true, 2);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let flags = read_static_field(
            &mut ctx,
            FLAGS,
            "DEFAULT_FLAGS",
            "Lnet/minecraft/world/flag/FeatureFlagSet;",
        );
        let menu = ctx.rt.new_instance(MENU, 0);
        invoke(&mut ctx,MENU,"<init>","(Lnet/minecraft/world/inventory/MenuType$MenuSupplier;Lnet/minecraft/world/flag/FeatureFlagSet;)V",false,&[menu,plain,flags]);
        let inventory = ctx
            .rt
            .new_instance("net/minecraft/world/entity/player/Inventory", 0);
        let data = ctx
            .rt
            .new_instance("net/minecraft/network/RegistryFriendlyByteBuf", 0);
        let result=invoke(&mut ctx,EXTENSION,"create","(ILnet/minecraft/world/entity/player/Inventory;Lnet/minecraft/network/RegistryFriendlyByteBuf;)Ljava/lang/Object;",false,&[menu,V::I(22),inventory,data]);
        assert_eq!(ctx.rt.get_field(result, "windowId"), V::I(22));
        assert_eq!(ctx.rt.get_field(result, "extraData"), V::NULL);
        let broken=invoke(&mut ctx,EXTENSION,"create","(Lnet/neoforged/neoforge/network/IContainerFactory;)Lnet/minecraft/world/inventory/MenuType;",true,&[failure]);
        invoke(&mut ctx,MENU,"create","(ILnet/minecraft/world/entity/player/Inventory;)Lnet/minecraft/world/inventory/AbstractContainerMenu;",false,&[broken,V::I(3),inventory]);
        let error = ctx.rt.take_exception().expect("factory exception");
        assert_eq!(ctx.rt.class_of(error), Some("example/MenuFailure"));
    }

    #[test]
    fn container_factory_default_method_supplies_the_third_null_argument() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let counter = rt.new_instance("example/Counter", 0);
        let factory = factory(&mut rt, counter, true, 1);
        let inventory = rt.new_instance("net/minecraft/world/entity/player/Inventory", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for owner in [FACTORY, SUPPLIER] {
            let result = invoke(
                &mut ctx,
                owner,
                "create",
                CREATE2,
                false,
                &[factory, V::I(5), inventory],
            );
            assert!(ctx.rt.pending_exception.is_none());
            assert_eq!(ctx.rt.get_field(result, "argumentCount"), V::I(3));
            assert_eq!(ctx.rt.get_field(result, "extraData"), V::NULL);
        }
    }
}
