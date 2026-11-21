//! Minecraft 1.21.1 NoteBlockInstrument metadata (`dud`) and actual sound references.
//! Pitched playback, note-block behavior, and custom-head sound extraction are outside this module.

use crate::RegistryStore;
use mmga_classfile::MemberRef;
use mmga_runtime::{Data, Rt, V};

const INSTRUMENT: &str = "net/minecraft/world/level/block/state/properties/NoteBlockInstrument";
const TYPE: &str = "net/minecraft/world/level/block/state/properties/NoteBlockInstrument$Type";
const TYPES: [&str; 3] = ["BASE_BLOCK", "MOB_HEAD", "CUSTOM"];
const NAMES: [&str; 23] = [
    "HARP",
    "BASEDRUM",
    "SNARE",
    "HAT",
    "BASS",
    "FLUTE",
    "BELL",
    "GUITAR",
    "CHIME",
    "XYLOPHONE",
    "IRON_XYLOPHONE",
    "COW_BELL",
    "DIDGERIDOO",
    "BIT",
    "BANJO",
    "PLING",
    "ZOMBIE",
    "SKELETON",
    "CREEPER",
    "DRAGON",
    "WITHER_SKELETON",
    "PIGLIN",
    "CUSTOM_HEAD",
];
const SOUNDS: [&str; 23] = [
    "NOTE_BLOCK_HARP",
    "NOTE_BLOCK_BASEDRUM",
    "NOTE_BLOCK_SNARE",
    "NOTE_BLOCK_HAT",
    "NOTE_BLOCK_BASS",
    "NOTE_BLOCK_FLUTE",
    "NOTE_BLOCK_BELL",
    "NOTE_BLOCK_GUITAR",
    "NOTE_BLOCK_CHIME",
    "NOTE_BLOCK_XYLOPHONE",
    "NOTE_BLOCK_IRON_XYLOPHONE",
    "NOTE_BLOCK_COW_BELL",
    "NOTE_BLOCK_DIDGERIDOO",
    "NOTE_BLOCK_BIT",
    "NOTE_BLOCK_BANJO",
    "NOTE_BLOCK_PLING",
    "NOTE_BLOCK_IMITATE_ZOMBIE",
    "NOTE_BLOCK_IMITATE_SKELETON",
    "NOTE_BLOCK_IMITATE_CREEPER",
    "NOTE_BLOCK_IMITATE_ENDER_DRAGON",
    "NOTE_BLOCK_IMITATE_WITHER_SKELETON",
    "NOTE_BLOCK_IMITATE_PIGLIN",
    "UI_BUTTON_CLICK",
];

pub fn get_static_field(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    owner: &str,
    name: &str,
    desc: &str,
) -> Option<V> {
    if !matches!(owner, INSTRUMENT | TYPE) || desc != format!("L{owner};") {
        return None;
    }
    constant(rt, registries, owner, name)
}
pub fn call(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner == "net/minecraft/util/StringRepresentable"
        && !is_static
        && name == "getSerializedName"
        && desc == "()Ljava/lang/String;"
        && rt.class_of(*args.first()?) == Some(INSTRUMENT)
    {
        return Some(rt.get_field(args[0], "name"));
    }
    if !matches!(owner, INSTRUMENT | TYPE) {
        return None;
    }
    if is_static {
        if name == "values" && desc == format!("()[L{owner};") {
            let names = if owner == TYPE {
                &TYPES[..]
            } else {
                &NAMES[..]
            };
            let values = names
                .iter()
                .map(|name| constant(rt, registries, owner, name).unwrap())
                .collect();
            return Some(rt.alloc(format!("[L{owner};"), 0, Data::Array(values)));
        }
        if name == "valueOf" && desc == format!("(Ljava/lang/String;)L{owner};") {
            let name = *args.first()?;
            if name.is_null() {
                return Some(rt.throw_exception("java/lang/NullPointerException", "Name is null"));
            }
            let name = rt.str_of(name)?.to_owned();
            return Some(constant(rt, registries, owner, &name).unwrap_or_else(|| {
                rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    &format!("No enum constant {}.{name}", owner.replace('/', ".")),
                )
            }));
        }
        return None;
    }
    if owner != INSTRUMENT {
        return None;
    }
    let recv = *args.first()?;
    match (name, desc) {
        ("getSerializedName", "()Ljava/lang/String;") => Some(rt.get_field(recv, "name")),
        ("getSoundEvent", "()Lnet/minecraft/core/Holder;") => {
            Some(rt.get_field(recv, "soundEvent"))
        }
        ("isTunable", "()Z") => Some(V::bool(
            rt.get_field(recv, "type") == enum_type(rt, "BASE_BLOCK", 0),
        )),
        ("hasCustomSound", "()Z") => Some(V::bool(
            rt.get_field(recv, "type") == enum_type(rt, "CUSTOM", 2),
        )),
        ("worksAboveNoteBlock", "()Z") => Some(V::bool(
            rt.get_field(recv, "type") != enum_type(rt, "BASE_BLOCK", 0),
        )),
        _ => None,
    }
}
fn constant(rt: &mut Rt, registries: &mut RegistryStore, owner: &str, name: &str) -> Option<V> {
    let names = if owner == TYPE {
        &TYPES[..]
    } else {
        &NAMES[..]
    };
    let index = names.iter().position(|&candidate| candidate == name)?;
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    if owner == TYPE {
        return Some(enum_type(rt, name, index));
    }
    let sound = crate::sounds::get_static_field(
        rt,
        registries,
        "net/minecraft/sounds/SoundEvents",
        SOUNDS[index],
        "Lnet/minecraft/core/Holder$Reference;",
    )?;
    let kind = if index < 16 {
        0
    } else if index < 22 {
        1
    } else {
        2
    };
    let kind = enum_type(rt, TYPES[kind], kind);
    let value = rt.new_instance(INSTRUMENT, 0);
    let label = rt.intern_str(name);
    let serialized = rt.intern_str(&name.to_ascii_lowercase());
    rt.put_field(value, "$name", label);
    rt.put_field(value, "$ordinal", V::I(index as i32));
    rt.put_field(value, "name", serialized);
    rt.put_field(value, "soundEvent", sound);
    rt.put_field(value, "type", kind);
    rt.put_static(INSTRUMENT, name, value);
    Some(value)
}
fn enum_type(rt: &mut Rt, name: &str, index: usize) -> V {
    if let Some(value) = rt.get_static(TYPE, name) {
        return value;
    }
    let value = rt.new_instance(TYPE, 0);
    let label = rt.intern_str(name);
    rt.put_field(value, "$name", label);
    rt.put_field(value, "$ordinal", V::I(index as i32));
    rt.put_static(TYPE, name, value);
    value
}
pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    (matches!(sub_class, INSTRUMENT | TYPE)
        && matches!(
            super_class,
            "java/lang/Enum" | "java/lang/Comparable" | "java/io/Serializable"
        ))
        || (sub_class == INSTRUMENT && super_class == "net/minecraft/util/StringRepresentable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, Data, ExecCtx, HostApi, LinkedSpace};
    #[derive(Default)]
    struct Host(RegistryStore);
    impl HostApi for Host {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            super::get_static_field(rt, &mut self.0, owner, name, desc)
                .or_else(|| crate::sounds::get_static_field(rt, &mut self.0, owner, name, desc))
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, &mut self.0, member, is_static, args)
                .or_else(|| {
                    crate::minecraft::call_keys_authlib_and_libs(
                        rt,
                        &mut self.0,
                        member,
                        is_static,
                        args,
                    )
                })
                .or_else(|| crate::sounds::call_sound_event(rt, member, is_static, args))
        }
        fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
            super::is_subtype(sub_class, super_class)
                || crate::minecraft::is_subtype(sub_class, super_class)
        }
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
    fn all_instruments_keep_exact_sound_registry_values_and_type_flags() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let values = invoke(
            &mut ctx,
            INSTRUMENT,
            "values",
            "()[Lnet/minecraft/world/level/block/state/properties/NoteBlockInstrument;",
            true,
            &[],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let values = match &ctx.rt.get_obj(values).unwrap().data {
            Data::Array(v) => v.clone(),
            _ => panic!("array"),
        };
        assert_eq!(values.len(), 23);
        for (index, name) in NAMES.iter().enumerate() {
            let instrument = read_static_field(
                &mut ctx,
                INSTRUMENT,
                name,
                "Lnet/minecraft/world/level/block/state/properties/NoteBlockInstrument;",
            );
            assert_eq!(values[index], instrument);
            let serialized = invoke(
                &mut ctx,
                INSTRUMENT,
                "getSerializedName",
                "()Ljava/lang/String;",
                false,
                &[instrument],
            );
            assert_eq!(
                ctx.rt.str_of(serialized),
                Some(name.to_ascii_lowercase().as_str())
            );
            for (method, expected) in [
                ("isTunable", index < 16),
                ("hasCustomSound", index == 22),
                ("worksAboveNoteBlock", index >= 16),
            ] {
                assert_eq!(
                    invoke(&mut ctx, INSTRUMENT, method, "()Z", false, &[instrument]),
                    V::bool(expected)
                );
            }
            let holder = invoke(
                &mut ctx,
                INSTRUMENT,
                "getSoundEvent",
                "()Lnet/minecraft/core/Holder;",
                false,
                &[instrument],
            );
            assert_eq!(
                holder,
                read_static_field(
                    &mut ctx,
                    "net/minecraft/sounds/SoundEvents",
                    SOUNDS[index],
                    "Lnet/minecraft/core/Holder$Reference;"
                )
            );
            let sound = invoke(
                &mut ctx,
                "net/minecraft/core/Holder",
                "value",
                "()Ljava/lang/Object;",
                false,
                &[holder],
            );
            let location = invoke(
                &mut ctx,
                "net/minecraft/sounds/SoundEvent",
                "getLocation",
                "()Lnet/minecraft/resources/ResourceLocation;",
                false,
                &[sound],
            );
            let expected = if index == 22 {
                "minecraft:ui.button.click".to_owned()
            } else if index >= 16 {
                format!(
                    "minecraft:block.note_block.imitate.{}",
                    if index == 19 {
                        "ender_dragon".into()
                    } else {
                        name.to_ascii_lowercase()
                    }
                )
            } else {
                format!("minecraft:block.note_block.{}", name.to_ascii_lowercase())
            };
            assert_eq!(ctx.rt.handle_of(location).unwrap().2, expected);
        }
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(host.0.total_mod_entries(), 0);
        assert!(host
            .0
            .lookup("sound_event", "minecraft:block.note_block.basedrum")
            .is_some());
    }
    #[test]
    fn enum_types_values_and_unknown_names_obey_java_rules() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let values = invoke(
            &mut ctx,
            TYPE,
            "values",
            "()[Lnet/minecraft/world/level/block/state/properties/NoteBlockInstrument$Type;",
            true,
            &[],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let values = match &ctx.rt.get_obj(values).unwrap().data {
            Data::Array(v) => v.clone(),
            _ => panic!("array"),
        };
        assert_eq!(values.len(), 3);
        for (index, name) in TYPES.iter().enumerate() {
            assert_eq!(
                values[index],
                read_static_field(
                    &mut ctx,
                    TYPE,
                    name,
                    "Lnet/minecraft/world/level/block/state/properties/NoteBlockInstrument$Type;"
                )
            );
        }
        let bad = ctx.rt.intern_str("DRUM");
        invoke(&mut ctx,INSTRUMENT,"valueOf","(Ljava/lang/String;)Lnet/minecraft/world/level/block/state/properties/NoteBlockInstrument;",true,&[bad]);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/IllegalArgumentException")
        );
    }
}
