//! Minecraft 1.21.1 DyeColor constants, verified against Mojang class `cti`.

use mmga_classfile::MemberRef;
use mmga_runtime::{Data, Rt, V};

const DYE: &str = "net/minecraft/world/item/DyeColor";
const DYE_DESC: &str = "Lnet/minecraft/world/item/DyeColor;";
const MAP_COLOR: &str = "net/minecraft/world/level/material/MapColor";
const NAMES: [&str; 16] = [
    "WHITE",
    "ORANGE",
    "MAGENTA",
    "LIGHT_BLUE",
    "YELLOW",
    "LIME",
    "PINK",
    "GRAY",
    "LIGHT_GRAY",
    "CYAN",
    "PURPLE",
    "BLUE",
    "BROWN",
    "GREEN",
    "RED",
    "BLACK",
];

// Texture RGB, canonical MapColor field, firework RGB, text RGB. Texture RGB is
// made opaque in the Java constructor; firework and text remain 24-bit colors.
const COLORS: [(i32, &str, i32, i32); 16] = [
    (16383998, "SNOW", 15790320, 16777215),
    (16351261, "COLOR_ORANGE", 15435844, 16738335),
    (13061821, "COLOR_MAGENTA", 12801229, 16711935),
    (3847130, "COLOR_LIGHT_BLUE", 6719955, 10141901),
    (16701501, "COLOR_YELLOW", 14602026, 16776960),
    (8439583, "COLOR_LIGHT_GREEN", 4312372, 12582656),
    (15961002, "COLOR_PINK", 14188952, 16738740),
    (4673362, "COLOR_GRAY", 4408131, 8421504),
    (10329495, "COLOR_LIGHT_GRAY", 11250603, 13882323),
    (1481884, "COLOR_CYAN", 2651799, 65535),
    (8991416, "COLOR_PURPLE", 8073150, 10494192),
    (3949738, "COLOR_BLUE", 2437522, 255),
    (8606770, "COLOR_BROWN", 5320730, 9127187),
    (6192150, "COLOR_GREEN", 3887386, 65280),
    (11546150, "COLOR_RED", 11743532, 16711680),
    (1908001, "COLOR_BLACK", 1973019, 0),
];

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != DYE || desc != DYE_DESC {
        return None;
    }
    let id = NAMES.iter().position(|&candidate| candidate == name)?;
    Some(constant(rt, id))
}
pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner == "net/minecraft/util/StringRepresentable"
        && !is_static
        && name == "getSerializedName"
        && desc == "()Ljava/lang/String;"
        && rt.class_of(*args.first()?) == Some(DYE)
    {
        return Some(rt.get_field(args[0], "name"));
    }
    if owner != DYE {
        return None;
    }
    match (name,desc,is_static) {
        ("values","()[Lnet/minecraft/world/item/DyeColor;",true) => {
            let values=(0..16).map(|id|constant(rt,id)).collect();
            Some(rt.alloc("[Lnet/minecraft/world/item/DyeColor;",0,Data::Array(values)))
        }
        ("valueOf","(Ljava/lang/String;)Lnet/minecraft/world/item/DyeColor;",true) => {
            let name=*args.first()?;
            if name.is_null(){return Some(rt.throw_exception("java/lang/NullPointerException","Name is null"));}
            let name=rt.str_of(name)?.to_owned();
            Some(if let Some(id)=NAMES.iter().position(|&candidate|candidate==name){constant(rt,id)}else{
                rt.throw_exception("java/lang/IllegalArgumentException",&format!("No enum constant net.minecraft.world.item.DyeColor.{name}"))
            })
        }
        ("byId","(I)Lnet/minecraft/world/item/DyeColor;",true) => {
            let id=args.first()?.i();
            Some(constant(rt,if (0..16).contains(&id){id as usize}else{0}))
        }
        ("byName","(Ljava/lang/String;Lnet/minecraft/world/item/DyeColor;)Lnet/minecraft/world/item/DyeColor;",true) => {
            let id=rt.str_of(*args.first()?).and_then(|name|NAMES.iter().position(|candidate|candidate.to_ascii_lowercase()==name));
            Some(match id {Some(id)=>constant(rt,id),None=>*args.get(1)?})
        }
        ("byFireworkColor","(I)Lnet/minecraft/world/item/DyeColor;",true) => {
            let rgb=args.first()?.i();
            Some(COLORS.iter().position(|(_,_,firework,_)|*firework==rgb).map_or(V::NULL,|id|constant(rt,id)))
        }
        ("getId","()I",false) => Some(rt.get_field(*args.first()?,"id")),
        ("getName"|"getSerializedName"|"toString","()Ljava/lang/String;",false) => Some(rt.get_field(*args.first()?,"name")),
        ("getTextureDiffuseColor","()I",false) => Some(rt.get_field(*args.first()?,"textureDiffuseColor")),
        ("getMapColor","()Lnet/minecraft/world/level/material/MapColor;",false) => Some(rt.get_field(*args.first()?,"mapColor")),
        ("getFireworkColor","()I",false) => Some(rt.get_field(*args.first()?,"fireworkColor")),
        ("getTextColor","()I",false) => Some(rt.get_field(*args.first()?,"textColor")),
        _=>None,
    }
}

fn constant(rt: &mut Rt, id: usize) -> V {
    let name = NAMES[id];
    if let Some(value) = rt.get_static(DYE, name) {
        return value;
    }
    let (texture, map_name, firework, text) = COLORS[id];
    let map = if let Some(value) = rt.get_static(MAP_COLOR, map_name) {
        value
    } else {
        let value = crate::minecraft::get_static_field(rt, MAP_COLOR, map_name)
            .expect("known 1.21.1 MapColor field");
        rt.put_static(MAP_COLOR, map_name, value);
        value
    };
    let color = rt.new_instance(DYE, 0);
    let enum_name = rt.intern_str(name);
    let serialized = rt.intern_str(&name.to_ascii_lowercase());
    rt.put_field(color, "$name", enum_name);
    rt.put_field(color, "$ordinal", V::I(id as i32));
    rt.put_field(color, "id", V::I(id as i32));
    rt.put_field(color, "name", serialized);
    rt.put_field(color, "mapColor", map);
    rt.put_field(
        color,
        "textureDiffuseColor",
        V::I((0xff000000u32 | texture as u32) as i32),
    );
    rt.put_field(color, "fireworkColor", V::I(firework));
    rt.put_field(color, "textColor", V::I(text));
    rt.put_static(DYE, name, color);
    color
}
pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    sub_class == DYE
        && matches!(
            super_class,
            "java/lang/Enum" | "java/lang/Comparable" | "net/minecraft/util/StringRepresentable"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, Data, ExecCtx, HostApi, LinkedSpace};
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
                .or_else(|| crate::minecraft::get_static_field(rt, owner, name))
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, member, is_static, args)
        }
        fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
            super::is_subtype(sub_class, super_class)
        }
    }
    fn invoke(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, is_static: bool, args: &[V]) -> V {
        invoke_member(
            ctx,
            if is_static {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(DYE, name, desc),
            args,
        )
    }
    #[test]
    fn all_dye_constants_keep_exact_colors_and_canonical_map_color() {
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
            (16383998, "SNOW", 15790320, 16777215),
            (16351261, "COLOR_ORANGE", 15435844, 16738335),
            (13061821, "COLOR_MAGENTA", 12801229, 16711935),
            (3847130, "COLOR_LIGHT_BLUE", 6719955, 10141901),
            (16701501, "COLOR_YELLOW", 14602026, 16776960),
            (8439583, "COLOR_LIGHT_GREEN", 4312372, 12582656),
            (15961002, "COLOR_PINK", 14188952, 16738740),
            (4673362, "COLOR_GRAY", 4408131, 8421504),
            (10329495, "COLOR_LIGHT_GRAY", 11250603, 13882323),
            (1481884, "COLOR_CYAN", 2651799, 65535),
            (8991416, "COLOR_PURPLE", 8073150, 10494192),
            (3949738, "COLOR_BLUE", 2437522, 255),
            (8606770, "COLOR_BROWN", 5320730, 9127187),
            (6192150, "COLOR_GREEN", 3887386, 65280),
            (11546150, "COLOR_RED", 11743532, 16711680),
            (1908001, "COLOR_BLACK", 1973019, 0),
        ];
        for (id, (name, (texture, map_name, firework, text))) in
            NAMES.iter().zip(expected).enumerate()
        {
            let color = read_static_field(&mut ctx, DYE, name, DYE_DESC);
            assert!(ctx.rt.pending_exception.is_none(), "missing {name}");
            assert_eq!(color, read_static_field(&mut ctx, DYE, name, DYE_DESC));
            assert_eq!(
                invoke(&mut ctx, "getId", "()I", false, &[color]),
                V::I(id as i32)
            );
            assert_eq!(
                invoke(&mut ctx, "getTextureDiffuseColor", "()I", false, &[color]),
                V::I((0xff000000u32 | texture as u32) as i32)
            );
            assert_eq!(
                invoke(&mut ctx, "getFireworkColor", "()I", false, &[color]),
                V::I(firework)
            );
            assert_eq!(
                invoke(&mut ctx, "getTextColor", "()I", false, &[color]),
                V::I(text)
            );
            let map = invoke(
                &mut ctx,
                "getMapColor",
                "()Lnet/minecraft/world/level/material/MapColor;",
                false,
                &[color],
            );
            assert_eq!(
                map,
                read_static_field(
                    &mut ctx,
                    MAP_COLOR,
                    map_name,
                    "Lnet/minecraft/world/level/material/MapColor;"
                )
            );
            assert_eq!(
                invoke(
                    &mut ctx,
                    "byFireworkColor",
                    "(I)Lnet/minecraft/world/item/DyeColor;",
                    true,
                    &[V::I(firework)]
                ),
                color
            );
            let label = invoke(
                &mut ctx,
                "toString",
                "()Ljava/lang/String;",
                false,
                &[color],
            );
            assert_eq!(
                ctx.rt.str_of(label),
                Some(name.to_ascii_lowercase().as_str())
            );
        }
    }
    #[test]
    fn dye_lookup_preserves_vanilla_fallbacks_and_values_copy() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let first = invoke(
            &mut ctx,
            "values",
            "()[Lnet/minecraft/world/item/DyeColor;",
            true,
            &[],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let values = match &ctx.rt.get_obj(first).unwrap().data {
            Data::Array(values) => values.clone(),
            _ => panic!("array"),
        };
        assert_eq!(values.len(), 16);
        if let Data::Array(array) = &mut ctx.rt.get_obj_mut(first).unwrap().data {
            array[0] = V::NULL;
        }
        let next = invoke(
            &mut ctx,
            "values",
            "()[Lnet/minecraft/world/item/DyeColor;",
            true,
            &[],
        );
        assert!(matches!(&ctx.rt.get_obj(next).unwrap().data,Data::Array(array) if array==&values));
        for index in [-1, 16, 99] {
            assert_eq!(
                invoke(
                    &mut ctx,
                    "byId",
                    "(I)Lnet/minecraft/world/item/DyeColor;",
                    true,
                    &[V::I(index)]
                ),
                values[0]
            );
        }
        let invalid = ctx.rt.new_str("not_dye");
        assert_eq!(invoke(&mut ctx,"byName","(Ljava/lang/String;Lnet/minecraft/world/item/DyeColor;)Lnet/minecraft/world/item/DyeColor;",true,&[invalid,V::NULL]),V::NULL);
        assert_eq!(
            invoke(
                &mut ctx,
                "byFireworkColor",
                "(I)Lnet/minecraft/world/item/DyeColor;",
                true,
                &[V::I(-1)]
            ),
            V::NULL
        );
        assert!(ctx.rt.pending_exception.is_none());
    }
}
