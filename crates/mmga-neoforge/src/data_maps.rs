//! Retained NeoForge data map type metadata and executable FurnaceFuel codecs.
//! Source commit 89e887b367ab0c11cb58ce8500920efadc17b803: DataMapType blob
//! 3417996a3f539c752c18c48775b631956226d341; FurnaceFuel blob
//! 044d7a0ed63b707ea35043cf3d95d6b5b90b365c; NeoForgeDataMaps blob
//! 4fc524b3624e6c738bddcf3f201f8e4d2ac38aef. Data loading/world queries are separate.

use mmga_classfile::MemberRef;
use mmga_runtime::{ClosureData, ClosureTarget, Data, LinkedSpace, NullHost, Rt, V};

const TYPE: &str = "net/neoforged/neoforge/registries/datamaps/DataMapType";
const BUILDER: &str = "net/neoforged/neoforge/registries/datamaps/DataMapType$Builder";
const MAPS: &str = "net/neoforged/neoforge/registries/datamaps/builtin/NeoForgeDataMaps";
const FUEL: &str = "net/neoforged/neoforge/registries/datamaps/builtin/FurnaceFuel";
const CODEC: &str = "com/mojang/serialization/Codec";

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    let fuel_codec = owner == FUEL
        && matches!(name, "CODEC" | "BURN_TIME_CODEC")
        && desc == "Lcom/mojang/serialization/Codec;";
    let furnace_type = owner == MAPS
        && name == "FURNACE_FUELS"
        && desc == "Lnet/neoforged/neoforge/registries/datamaps/DataMapType;";
    if !fuel_codec && !furnace_type {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    if furnace_type {
        let codec = get_static_field(rt, FUEL, "CODEC", "Lcom/mojang/serialization/Codec;")?;
        let network = get_static_field(
            rt,
            FUEL,
            "BURN_TIME_CODEC",
            "Lcom/mojang/serialization/Codec;",
        )?;
        let registry = crate::minecraft::new_resource_key(rt, "minecraft:root", "minecraft:item");
        let id = rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "neoforge:furnace_fuels",
        );
        let typ = rt.new_instance(TYPE, 0);
        if !initialize_type(rt, typ, &[registry, id, codec, network, V::FALSE]) {
            return Some(V::NULL);
        }
        rt.put_static(owner, name, typ);
        return Some(typ);
    }
    let positive = crate::dfu::get_static_field(
        rt,
        "net/minecraft/util/ExtraCodecs",
        "POSITIVE_INT",
        "Lcom/mojang/serialization/Codec;",
    )?;
    let getter = fuel_function(rt, false);
    let constructor = fuel_function(rt, true);
    let value = if name == "BURN_TIME_CODEC" {
        compose(rt,"xmap","(Ljava/util/function/Function;Ljava/util/function/Function;)Lcom/mojang/serialization/Codec;",false,&[positive,constructor,getter])?
    } else {
        let field = crate::dfu::field_codec(rt, positive, "burn_time");
        let map = crate::dfu::record_map_codec(rt, &[(field, getter)], constructor);
        let record = crate::dfu::codec_from_map(rt, map);
        let inline = get_static_field(
            rt,
            FUEL,
            "BURN_TIME_CODEC",
            "Lcom/mojang/serialization/Codec;",
        )?;
        compose(rt,"withAlternative","(Lcom/mojang/serialization/Codec;Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/Codec;",true,&[record,inline])?
    };
    rt.put_static(owner, name, value);
    Some(value)
}
pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if is_static {
        if owner==TYPE&&name=="builder"&&desc=="(Lnet/minecraft/resources/ResourceLocation;Lnet/minecraft/resources/ResourceKey;Lcom/mojang/serialization/Codec;)Lnet/neoforged/neoforge/registries/datamaps/DataMapType$Builder;" {
            let builder=rt.new_instance(BUILDER,0);
            initialize_builder(rt,builder,&[args[1],args[0],args[2]]);return Some(builder);
        }
        return None;
    }
    let recv = *args.first()?;
    if owner == FUEL {
        if rt.class_of(recv) != Some(FUEL) {
            return Some(rt.throw_exception(
                "java/lang/ClassCastException",
                "FurnaceFuel method requires FurnaceFuel record",
            ));
        }
        return match (name, desc) {
            ("<init>", "(I)V") => {
                rt.put_field(recv, "burnTime", *args.get(1)?);
                Some(V::Void)
            }
            ("burnTime", "()I") => Some(rt.get_field(recv, "burnTime")),
            ("equals", "(Ljava/lang/Object;)Z") => Some(V::bool(
                rt.class_of(args[1]) == Some(FUEL)
                    && rt.get_field(recv, "burnTime") == rt.get_field(args[1], "burnTime"),
            )),
            ("hashCode", "()I") => Some(rt.get_field(recv, "burnTime")),
            ("toString", "()Ljava/lang/String;") => Some(rt.new_str(format!(
                "FurnaceFuel[burnTime={}]",
                rt.get_field(recv, "burnTime").i()
            ))),
            _ => None,
        };
    }
    if owner == BUILDER {
        return match(name,desc){
            ("<init>","(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;Lcom/mojang/serialization/Codec;)V")=>{
                initialize_builder(rt,recv,&args[1..]);Some(V::Void)
            }
            ("synced","(Lcom/mojang/serialization/Codec;Z)Lnet/neoforged/neoforge/registries/datamaps/DataMapType$Builder;")=>{
                rt.put_field(recv,"networkCodec",*args.get(1)?);rt.put_field(recv,"mandatorySync",*args.get(2)?);Some(recv)
            }
            ("build","()Lnet/neoforged/neoforge/registries/datamaps/DataMapType;")=>{
                let fields=type_fields(rt,recv);let typ=rt.new_instance(TYPE,0);
                Some(if initialize_type(rt,typ,&fields){typ}else{V::NULL})
            }
            _=>None,
        };
    }
    if owner == TYPE {
        return match(name,desc){
            ("<init>","(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;Lcom/mojang/serialization/Codec;Lcom/mojang/serialization/Codec;Z)V")=>{
                Some(if initialize_type(rt,recv,&args[1..]){V::Void}else{V::NULL})
            }
            ("registryKey","()Lnet/minecraft/resources/ResourceKey;")|( "id","()Lnet/minecraft/resources/ResourceLocation;")
            |("codec"|"networkCodec","()Lcom/mojang/serialization/Codec;")|("mandatorySync","()Z")=>Some(rt.get_field(recv,name)),
            _=>None,
        };
    }
    // A metadata value is not loaded data. Returning an empty map/default here
    // would misrepresent conditions, reload order and actual registry entries.
    if matches!(name, "getDataMap" | "getData")
        && args
            .get(1)
            .is_some_and(|value| rt.class_of(*value) == Some(TYPE))
    {
        return Some(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "MMGA does not implement data-map loading or registry/holder data queries",
        ));
    }
    None
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    sub == FUEL && sup == "java/lang/Record"
}

fn initialize_builder(rt: &mut Rt, builder: V, args: &[V]) {
    for (name, value) in [
        ("registryKey", args[0]),
        ("id", args[1]),
        ("codec", args[2]),
        ("networkCodec", V::NULL),
        ("mandatorySync", V::FALSE),
    ] {
        rt.put_field(builder, name, value);
    }
}
fn type_fields(rt: &Rt, value: V) -> [V; 5] {
    [
        rt.get_field(value, "registryKey"),
        rt.get_field(value, "id"),
        rt.get_field(value, "codec"),
        rt.get_field(value, "networkCodec"),
        rt.get_field(value, "mandatorySync"),
    ]
}
fn initialize_type(rt: &mut Rt, typ: V, args: &[V]) -> bool {
    if args[3].is_null() && args[4].i() != 0 {
        rt.throw_exception(
            "java/lang/IllegalArgumentException",
            "Mandatory sync cannot be enabled when the attachment isn't synchronized",
        );
        return false;
    }
    for (index, name, message) in [
        (0, "registryKey", "registryKey must not be null"),
        (1, "id", "id must not be null"),
        (2, "codec", "codec must not be null"),
    ] {
        if args[index].is_null() {
            rt.throw_exception("java/lang/NullPointerException", message);
            return false;
        }
        rt.put_field(typ, name, args[index]);
    }
    rt.put_field(typ, "networkCodec", args[3]);
    rt.put_field(typ, "mandatorySync", V::bool(args[4].i() != 0));
    true
}
fn fuel_function(rt: &mut Rt, constructor: bool) -> V {
    let member = if constructor {
        MemberRef::new(FUEL, "<init>", "(I)V")
    } else {
        MemberRef::new(FUEL, "burnTime", "()I")
    };
    rt.alloc("java/util/function/Function",0,Data::Closure(Box::new(ClosureData{
        interface_class:"java/util/function/Function".into(),sam_name:"apply".into(),sam_desc:"(Ljava/lang/Object;)Ljava/lang/Object;".into(),
        instantiated_desc:if constructor{"(Ljava/lang/Integer;)Lnet/neoforged/neoforge/registries/datamaps/builtin/FurnaceFuel;"}else{"(Lnet/neoforged/neoforge/registries/datamaps/builtin/FurnaceFuel;)Ljava/lang/Integer;"}.into(),
        captures:Vec::new(),target:ClosureTarget::MethodHandle{handle_kind:if constructor{8}else{5},member},
    })))
}
fn compose(rt: &mut Rt, name: &str, desc: &str, is_static: bool, args: &[V]) -> Option<V> {
    // Only composition operations run here. They retain callbacks without
    // invoking them; encode/decode later executes against the real VM host.
    crate::dfu::call(
        rt,
        &LinkedSpace::new(),
        &mut NullHost,
        &MemberRef::new(CODEC, name, desc),
        is_static,
        args,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, ExecCtx, HostApi, LinkedSpace};
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
                .or_else(|| crate::dfu::get_static_field(rt, owner, name, desc))
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, member, is_static, args)
                .or_else(|| crate::dfu::call(rt, linked, self, member, is_static, args))
        }
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            super::is_subtype(sub, sup) || crate::dfu::is_subtype(sub, sup)
        }
    }
    fn invoke(
        ctx: &mut ExecCtx<'_>,
        owner: &str,
        name: &str,
        desc: &str,
        stat: bool,
        args: &[V],
    ) -> V {
        invoke_member(
            ctx,
            if stat {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    fn parse_json(ctx: &mut ExecCtx<'_>, text: &str) -> V {
        let text = ctx.rt.new_str(text);
        invoke(
            ctx,
            "com/google/gson/JsonParser",
            "parseString",
            "(Ljava/lang/String;)Lcom/google/gson/JsonElement;",
            true,
            &[text],
        )
    }
    fn result_value(ctx: &mut ExecCtx<'_>, result: V) -> V {
        invoke(
            ctx,
            "com/mojang/serialization/DataResult",
            "getOrThrow",
            "()Ljava/lang/Object;",
            false,
            &[result],
        )
    }
    fn assert_data_error(ctx: &mut ExecCtx<'_>, result: V) {
        let error = invoke(
            ctx,
            "com/mojang/serialization/DataResult",
            "error",
            "()Ljava/util/Optional;",
            false,
            &[result],
        );
        assert_eq!(
            invoke(
                ctx,
                "java/util/Optional",
                "isPresent",
                "()Z",
                false,
                &[error]
            ),
            V::TRUE
        );
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn codec_mapper_rejects_wrong_record_type_and_unimplemented_dynamic_ops() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let codec = read_static_field(&mut ctx, FUEL, "CODEC", "Lcom/mojang/serialization/Codec;");
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let wrong = ctx.rt.new_instance("test/WrongRecord", 0);
        invoke(&mut ctx,CODEC,"encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,ops,wrong]);
        let error = ctx
            .rt
            .take_exception()
            .expect("a typed FurnaceFuel getter must not read an absent field as a default");
        assert_eq!(ctx.rt.class_of(error), Some("java/lang/ClassCastException"));
        let unknown_ops = ctx.rt.new_instance("test/DynamicOps", 0);
        let input = parse_json(&mut ctx, "200");
        invoke(&mut ctx,CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,unknown_ops,input]);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
    }
    #[test]
    fn furnace_map_keeps_real_registry_codecs_sync_flags_and_json_forms() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let typ = read_static_field(
            &mut ctx,
            MAPS,
            "FURNACE_FUELS",
            "Lnet/neoforged/neoforge/registries/datamaps/DataMapType;",
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "FURNACE_FUELS must expose real type metadata"
        );
        assert_eq!(
            typ,
            read_static_field(
                &mut ctx,
                MAPS,
                "FURNACE_FUELS",
                "Lnet/neoforged/neoforge/registries/datamaps/DataMapType;"
            )
        );
        let registry = invoke(
            &mut ctx,
            TYPE,
            "registryKey",
            "()Lnet/minecraft/resources/ResourceKey;",
            false,
            &[typ],
        );
        assert_eq!(
            crate::minecraft::resource_key_parts(ctx.rt, registry),
            Some(("minecraft:root".into(), "minecraft:item".into()))
        );
        let id = invoke(
            &mut ctx,
            TYPE,
            "id",
            "()Lnet/minecraft/resources/ResourceLocation;",
            false,
            &[typ],
        );
        assert_eq!(ctx.rt.handle_of(id).unwrap().2, "neoforge:furnace_fuels");
        let codec = invoke(
            &mut ctx,
            TYPE,
            "codec",
            "()Lcom/mojang/serialization/Codec;",
            false,
            &[typ],
        );
        let network = invoke(
            &mut ctx,
            TYPE,
            "networkCodec",
            "()Lcom/mojang/serialization/Codec;",
            false,
            &[typ],
        );
        assert_eq!(
            codec,
            read_static_field(&mut ctx, FUEL, "CODEC", "Lcom/mojang/serialization/Codec;")
        );
        assert_eq!(
            network,
            read_static_field(
                &mut ctx,
                FUEL,
                "BURN_TIME_CODEC",
                "Lcom/mojang/serialization/Codec;"
            )
        );
        assert_eq!(
            invoke(&mut ctx, TYPE, "mandatorySync", "()Z", false, &[typ]),
            V::FALSE
        );
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        for json in ["200", "{\"burn_time\":200}"] {
            let input = parse_json(&mut ctx, json);
            let result=invoke(&mut ctx,CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,ops,input]);
            let fuel = result_value(&mut ctx, result);
            assert_eq!(
                invoke(&mut ctx, FUEL, "burnTime", "()I", false, &[fuel]),
                V::I(200)
            );
            for (codec, expected) in [(codec, "{\"burn_time\":200}"), (network, "200")] {
                let result=invoke(&mut ctx,CODEC,"encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,ops,fuel]);
                let encoded = result_value(&mut ctx, result);
                let text = invoke(
                    &mut ctx,
                    "com/google/gson/JsonElement",
                    "toString",
                    "()Ljava/lang/String;",
                    false,
                    &[encoded],
                );
                assert_eq!(ctx.rt.str_of(text), Some(expected));
            }
        }
        for json in ["0", "-1", "{}", "{\"burn_time\":0}", "\"200\""] {
            let input = parse_json(&mut ctx, json);
            let result=invoke(&mut ctx,CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,ops,input]);
            assert_data_error(&mut ctx, result);
        }
        let negative = ctx.rt.new_instance(FUEL, 0);
        invoke(
            &mut ctx,
            FUEL,
            "<init>",
            "(I)V",
            false,
            &[negative, V::I(-1)],
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "FurnaceFuel constructor itself permits negative values"
        );
        let result=invoke(&mut ctx,CODEC,"encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,ops,negative]);
        assert_data_error(&mut ctx, result);
    }
    #[test]
    fn builder_snapshots_original_references_and_validates_only_on_build() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let registry =
            crate::minecraft::new_resource_key(ctx.rt, "minecraft:root", "example:registry");
        let id = ctx.rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "example:map",
        );
        let codec = ctx.rt.new_instance(CODEC, 0);
        let network = ctx.rt.new_instance(CODEC, 0);
        let builder=invoke(&mut ctx,TYPE,"builder","(Lnet/minecraft/resources/ResourceLocation;Lnet/minecraft/resources/ResourceKey;Lcom/mojang/serialization/Codec;)Lnet/neoforged/neoforge/registries/datamaps/DataMapType$Builder;",true,&[id,registry,codec]);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "builder metadata must retain supplied references"
        );
        assert_eq!(invoke(&mut ctx,BUILDER,"synced","(Lcom/mojang/serialization/Codec;Z)Lnet/neoforged/neoforge/registries/datamaps/DataMapType$Builder;",false,&[builder,network,V::TRUE]),builder);
        let first = invoke(
            &mut ctx,
            BUILDER,
            "build",
            "()Lnet/neoforged/neoforge/registries/datamaps/DataMapType;",
            false,
            &[builder],
        );
        invoke(&mut ctx,BUILDER,"synced","(Lcom/mojang/serialization/Codec;Z)Lnet/neoforged/neoforge/registries/datamaps/DataMapType$Builder;",false,&[builder,V::NULL,V::FALSE]);
        let second = invoke(
            &mut ctx,
            BUILDER,
            "build",
            "()Lnet/neoforged/neoforge/registries/datamaps/DataMapType;",
            false,
            &[builder],
        );
        assert_ne!(first, second);
        assert_eq!(
            invoke(
                &mut ctx,
                TYPE,
                "codec",
                "()Lcom/mojang/serialization/Codec;",
                false,
                &[first]
            ),
            codec
        );
        assert_eq!(
            invoke(
                &mut ctx,
                TYPE,
                "id",
                "()Lnet/minecraft/resources/ResourceLocation;",
                false,
                &[first]
            ),
            id
        );
        assert_eq!(
            invoke(
                &mut ctx,
                TYPE,
                "registryKey",
                "()Lnet/minecraft/resources/ResourceKey;",
                false,
                &[first]
            ),
            registry
        );
        assert_eq!(
            invoke(
                &mut ctx,
                TYPE,
                "networkCodec",
                "()Lcom/mojang/serialization/Codec;",
                false,
                &[first]
            ),
            network
        );
        assert_eq!(
            invoke(
                &mut ctx,
                TYPE,
                "networkCodec",
                "()Lcom/mojang/serialization/Codec;",
                false,
                &[second]
            ),
            V::NULL
        );
        let invalid=invoke(&mut ctx,TYPE,"builder","(Lnet/minecraft/resources/ResourceLocation;Lnet/minecraft/resources/ResourceKey;Lcom/mojang/serialization/Codec;)Lnet/neoforged/neoforge/registries/datamaps/DataMapType$Builder;",true,&[V::NULL,V::NULL,V::NULL]);
        assert!(ctx.rt.pending_exception.is_none());
        invoke(&mut ctx,BUILDER,"synced","(Lcom/mojang/serialization/Codec;Z)Lnet/neoforged/neoforge/registries/datamaps/DataMapType$Builder;",false,&[invalid,V::NULL,V::TRUE]);
        invoke(
            &mut ctx,
            BUILDER,
            "build",
            "()Lnet/neoforged/neoforge/registries/datamaps/DataMapType;",
            false,
            &[invalid],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/IllegalArgumentException")
        );
        invoke(&mut ctx,BUILDER,"synced","(Lcom/mojang/serialization/Codec;Z)Lnet/neoforged/neoforge/registries/datamaps/DataMapType$Builder;",false,&[invalid,V::NULL,V::FALSE]);
        invoke(
            &mut ctx,
            BUILDER,
            "build",
            "()Lnet/neoforged/neoforge/registries/datamaps/DataMapType;",
            false,
            &[invalid],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/NullPointerException")
        );
        assert!(get_static_field(
            ctx.rt,
            MAPS,
            "UNKNOWN_MAP",
            "Lnet/neoforged/neoforge/registries/datamaps/DataMapType;"
        )
        .is_none());
    }
}
