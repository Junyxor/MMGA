//! Minecraft 1.21.1 WoodType (`dup`) names, registration and supplied record fields.
//! Official client SHA-1 30c73b1c5da787909b2f73340419fdf13b9def88; mapping SHA-1
//! 2244b6f072256667bcd9a73df124d6c58de77992. Builtin blockset/sound dependencies
//! are unresolved here and explicitly fail when requested rather than returning null.

use mmga_classfile::MemberRef;
use mmga_runtime::{Data, Rt, V};
const WOOD: &str = "net/minecraft/world/level/block/state/properties/WoodType";
const DESC: &str = "Lnet/minecraft/world/level/block/state/properties/WoodType;";
const CONSTRUCTOR:&str="(Ljava/lang/String;Lnet/minecraft/world/level/block/state/properties/BlockSetType;Lnet/minecraft/world/level/block/SoundType;Lnet/minecraft/world/level/block/SoundType;Lnet/minecraft/sounds/SoundEvent;Lnet/minecraft/sounds/SoundEvent;)V";
const NAMES: [&str; 11] = [
    "OAK", "SPRUCE", "BIRCH", "ACACIA", "CHERRY", "JUNGLE", "DARK_OAK", "CRIMSON", "WARPED",
    "MANGROVE", "BAMBOO",
];
const FIELDS: [&str; 6] = [
    "name",
    "setType",
    "soundType",
    "hangingSignSoundType",
    "fenceGateClose",
    "fenceGateOpen",
];
pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != WOOD || desc != DESC || !NAMES.contains(&name) {
        return None;
    }
    initialize_types(rt);
    rt.get_static(WOOD, name)
}
pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if member.owner != WOOD {
        return None;
    }
    let (name, desc) = (member.name.as_str(), member.desc.as_str());
    if is_static {
        return match(name,desc){
            ("values","()Ljava/util/stream/Stream;")=>{
                let registry=initialize_types(rt);
                let Some(Data::Map(values))=rt.get_obj(registry).map(|object|&object.data) else{return Some(unsupported(rt,"WoodType registration map"));};
                let values=values.iter().map(|(_,value)|*value).collect();
                Some(rt.alloc("java/util/stream/Stream",0,Data::Stream(values)))
            }
            ("register","(Lnet/minecraft/world/level/block/state/properties/WoodType;)Lnet/minecraft/world/level/block/state/properties/WoodType;")=>{
                let value=*args.first()?;
                if value.is_null(){return Some(rt.throw_exception("java/lang/NullPointerException","WoodType.register(null)"));}
                if rt.class_of(value)!=Some(WOOD){return Some(rt.throw_exception("java/lang/ClassCastException","WoodType registration requires WoodType"));}
                let registry=initialize_types(rt);
                let name=rt.get_field(value,"name");
                let entries=match rt.get_obj(registry).map(|object|&object.data){Some(Data::Map(entries))=>entries.clone(),_=>return Some(unsupported(rt,"WoodType registration map"))};
                let old=entries.iter().position(|(key,_)|rt.val_eq(*key,name));
                if let Some(Data::Map(entries))=rt.get_obj_mut(registry).map(|object|&mut object.data){
                    if let Some(old)=old{entries[old].1=value;}else{entries.push((name,value));}
                }
                Some(value)
            }
            _=>None,
        };
    }
    let recv = *args.first()?;
    if rt.class_of(recv) != Some(WOOD) {
        return Some(rt.throw_exception(
            "java/lang/ClassCastException",
            "WoodType method requires WoodType",
        ));
    }
    if name == "<init>" && desc == CONSTRUCTOR {
        if args.len() != 7 {
            return None;
        }
        for (field, value) in FIELDS.into_iter().zip(&args[1..]) {
            rt.put_field(recv, field, *value);
        }
        rt.put_field(recv, "$unresolvedBuiltinFields", V::FALSE);
        return Some(V::Void);
    }
    if name=="<init>"&&desc=="(Ljava/lang/String;Lnet/minecraft/world/level/block/state/properties/BlockSetType;)V" {
        return Some(unsupported(rt,"WoodType default WOOD/HANGING_SIGN sound and fence-gate constructor dependencies"));
    }
    let field = match (name, desc) {
        ("name", "()Ljava/lang/String;") => Some("name"),
        ("setType", "()Lnet/minecraft/world/level/block/state/properties/BlockSetType;") => {
            Some("setType")
        }
        ("soundType" | "hangingSignSoundType", "()Lnet/minecraft/world/level/block/SoundType;") => {
            Some(name)
        }
        ("fenceGateClose" | "fenceGateOpen", "()Lnet/minecraft/sounds/SoundEvent;") => Some(name),
        _ => None,
    };
    if let Some(field) = field {
        if field != "name" && rt.get_field(recv, "$unresolvedBuiltinFields") == V::TRUE {
            return Some(unsupported(
                rt,
                &format!("WoodType builtin {field} dependency"),
            ));
        }
        return Some(rt.get_field(recv, field));
    }
    match (name, desc) {
        ("equals", "(Ljava/lang/Object;)Z") => {
            let other = *args.get(1)?;
            if recv == other {
                return Some(V::TRUE);
            }
            if rt.class_of(other) != Some(WOOD) {
                return Some(V::FALSE);
            }
            if !rt.val_eq(rt.get_field(recv, "name"), rt.get_field(other, "name")) {
                return Some(V::FALSE);
            }
            Some(unsupported(
                rt,
                "WoodType equality of nested blockset and sound metadata",
            ))
        }
        ("hashCode", "()I") => Some(unsupported(
            rt,
            "WoodType record hash with nested blockset and sound metadata",
        )),
        ("toString", "()Ljava/lang/String;") => Some(unsupported(
            rt,
            "WoodType record string with nested blockset and sound metadata",
        )),
        _ => None,
    }
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    sub == WOOD && sup == "java/lang/Record"
}

fn initialize_types(rt: &mut Rt) -> V {
    if let Some(registry) = rt.get_static("mmga/wood_types", "registry") {
        return registry;
    }
    // This is the complete ordered name registration in dup.<clinit>. Each
    // object explicitly marks the dependencies not implemented by this subset.
    let mut values = Vec::new();
    for name in NAMES {
        let value = rt.new_instance(WOOD, 0);
        let label = rt.intern_str(&name.to_ascii_lowercase());
        rt.put_field(value, "name", label);
        rt.put_field(value, "$unresolvedBuiltinFields", V::TRUE);
        rt.put_static(WOOD, name, value);
        values.push((label, value));
    }
    let registry = rt.alloc(
        "it/unimi/dsi/fastutil/objects/Object2ObjectArrayMap",
        0,
        Data::Map(values),
    );
    rt.put_static("mmga/wood_types", "registry", registry);
    registry
}
fn unsupported(rt: &mut Rt, reason: &str) -> V {
    rt.throw_exception(
        "java/lang/UnsupportedOperationException",
        &format!("MMGA does not implement {reason}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{
        invoke_member, read_instance_field, read_static_field, ExecCtx, HostApi, LinkedSpace,
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
            _: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, member, is_static, args)
        }
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            super::is_subtype(sub, sup)
        }
    }
    fn invoke(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, stat: bool, args: &[V]) -> V {
        invoke_member(
            ctx,
            if stat {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(WOOD, name, desc),
            args,
        )
    }
    fn values(ctx: &mut ExecCtx<'_>) -> Vec<V> {
        let stream = invoke(ctx, "values", "()Ljava/util/stream/Stream;", true, &[]);
        match &ctx.rt.get_obj(stream).unwrap().data {
            Data::Stream(values) => values.clone(),
            _ => panic!("WoodType stream"),
        }
    }
    #[test]
    fn builtin_names_preserve_all_eleven_registered_identities_and_reject_unresolved_fields() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let expected = [
            "OAK", "SPRUCE", "BIRCH", "ACACIA", "CHERRY", "JUNGLE", "DARK_OAK", "CRIMSON",
            "WARPED", "MANGROVE", "BAMBOO",
        ];
        let first = read_static_field(&mut ctx, WOOD, "OAK", DESC);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "real WoodType.OAK metadata missing"
        );
        let all = values(&mut ctx);
        assert_eq!(all.len(), 11);
        for (index, name) in expected.iter().enumerate() {
            let value = read_static_field(&mut ctx, WOOD, name, DESC);
            assert_eq!(value, all[index]);
            let label = invoke(&mut ctx, "name", "()Ljava/lang/String;", false, &[value]);
            assert_eq!(
                ctx.rt.str_of(label),
                Some(name.to_ascii_lowercase().as_str())
            );
        }
        invoke(
            &mut ctx,
            "setType",
            "()Lnet/minecraft/world/level/block/state/properties/BlockSetType;",
            false,
            &[first],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
        invoke(
            &mut ctx,
            "soundType",
            "()Lnet/minecraft/world/level/block/SoundType;",
            false,
            &[first],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
        invoke(&mut ctx, "hashCode", "()I", false, &[first]);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
        read_instance_field(
            &mut ctx,
            first,
            WOOD,
            "soundType",
            "Lnet/minecraft/world/level/block/SoundType;",
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
    }
    #[test]
    fn full_constructor_keeps_all_supplied_references_and_registration_replaces_only_map_value() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let original = read_static_field(&mut ctx, WOOD, "OAK", DESC);
        let name = ctx.rt.intern_str("oak");
        let set = ctx.rt.new_instance(
            "net/minecraft/world/level/block/state/properties/BlockSetType",
            0,
        );
        let sound = ctx
            .rt
            .new_instance("net/minecraft/world/level/block/SoundType", 0);
        let close = ctx.rt.new_instance("net/minecraft/sounds/SoundEvent", 0);
        let open = ctx.rt.new_instance("net/minecraft/sounds/SoundEvent", 0);
        let custom = ctx.rt.new_instance(WOOD, 0);
        invoke(
            &mut ctx,
            "<init>",
            CONSTRUCTOR,
            false,
            &[custom, name, set, sound, V::NULL, close, open],
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "canonical record ctor must retain caller fields"
        );
        for (field, desc, wanted) in [
            ("name", "()Ljava/lang/String;", name),
            (
                "setType",
                "()Lnet/minecraft/world/level/block/state/properties/BlockSetType;",
                set,
            ),
            (
                "soundType",
                "()Lnet/minecraft/world/level/block/SoundType;",
                sound,
            ),
            (
                "hangingSignSoundType",
                "()Lnet/minecraft/world/level/block/SoundType;",
                V::NULL,
            ),
            (
                "fenceGateClose",
                "()Lnet/minecraft/sounds/SoundEvent;",
                close,
            ),
            ("fenceGateOpen", "()Lnet/minecraft/sounds/SoundEvent;", open),
        ] {
            assert_eq!(invoke(&mut ctx, field, desc, false, &[custom]), wanted);
        }
        assert_eq!(invoke(&mut ctx,"register","(Lnet/minecraft/world/level/block/state/properties/WoodType;)Lnet/minecraft/world/level/block/state/properties/WoodType;",true,&[custom]),custom);
        let all = values(&mut ctx);
        assert_eq!(all.len(), 11);
        assert_eq!(all[0], custom);
        assert_eq!(read_static_field(&mut ctx, WOOD, "OAK", DESC), original);
        assert!(!get_static_field(ctx.rt, WOOD, "UNVERIFIED", DESC).is_some());
    }
}
