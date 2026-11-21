//! ItemContainerContents' bounded stream graph and empty container values.
//! Nonempty stack hashing/serialization still require item and component semantics.
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{
    invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, HostApi, LinkedSpace, Rt, V,
};

const CONTENTS: &str = "net/minecraft/world/item/component/ItemContainerContents";
const STACK: &str = "net/minecraft/world/item/ItemStack";
const CALLBACK: &str = "mmga/internal/ContainerItemsCodec";
const STREAM: &str = "net/minecraft/network/codec/StreamCodec";
const CODEC: &str = "com/mojang/serialization/Codec";
const SLOT: &str = "net/minecraft/world/item/component/ItemContainerContents$Slot";

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != CONTENTS {
        return None;
    }
    if name == "EMPTY" && desc == "Lnet/minecraft/world/item/component/ItemContainerContents;" {
        return Some(empty(rt));
    }
    if name == "CODEC" && desc == "Lcom/mojang/serialization/Codec;" {
        if let Some(value) = rt.get_static(owner, name) {
            return Some(value);
        }
        let value = container_codec(rt);
        rt.put_static(owner, name, value);
        return Some(value);
    }
    if name != "STREAM_CODEC" || desc != "Lnet/minecraft/network/codec/StreamCodec;" {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let child = crate::codecs::get_static_field(
        rt,
        STACK,
        "OPTIONAL_STREAM_CODEC",
        "Lnet/minecraft/network/codec/StreamCodec;",
    )?;
    let linked = LinkedSpace::new();
    let mut host = mmga_runtime::NullHost;
    let operation = crate::codecs::call(
        rt,
        &linked,
        &mut host,
        &MemberRef::new(
            "net/minecraft/network/codec/ByteBufCodecs",
            "list",
            "(I)Lnet/minecraft/network/codec/StreamCodec$CodecOperation;",
        ),
        true,
        &[V::I(256)],
    )?;
    let list = crate::codecs::call(
        rt,
        &linked,
        &mut host,
        &MemberRef::new(
            "net/minecraft/network/codec/StreamCodec$CodecOperation",
            "apply",
            "(Lnet/minecraft/network/codec/StreamCodec;)Lnet/minecraft/network/codec/StreamCodec;",
        ),
        false,
        &[operation, child],
    )?;
    let from = callback(
        rt,
        "fromList",
        "(Ljava/util/List;)Lnet/minecraft/world/item/component/ItemContainerContents;",
    );
    let to = callback(
        rt,
        "items",
        "(Lnet/minecraft/world/item/component/ItemContainerContents;)Ljava/util/List;",
    );
    let value=crate::codecs::call(rt,&linked,&mut host,&MemberRef::new(STREAM,"map","(Ljava/util/function/Function;Ljava/util/function/Function;)Lnet/minecraft/network/codec/StreamCodec;"),false,&[list,from,to])?;
    rt.put_static(owner, name, value);
    Some(value)
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
    let owner = if owner == "java/lang/Object"
        && !is_static
        && args
            .first()
            .is_some_and(|recv| rt.class_of(*recv) == Some(CONTENTS))
        && matches!(
            (name, desc),
            ("equals", "(Ljava/lang/Object;)Z") | ("hashCode", "()I")
        ) {
        CONTENTS
    } else {
        owner
    };
    let mut ctx = ExecCtx {
        rt,
        linked,
        host,
        depth: 0,
    };
    if owner == CALLBACK && is_static {
        return match (name, desc) {
            ("slotIndex", "(Lnet/minecraft/world/item/component/ItemContainerContents$Slot;)Ljava/lang/Object;") => {
                let index = ctx.rt.get_field(args[0], "index");
                Some(mmga_runtime::box_primitive(ctx.rt, "java/lang/Integer", index))
            }
            ("slotItem", "(Lnet/minecraft/world/item/component/ItemContainerContents$Slot;)Ljava/lang/Object;") => Some(ctx.rt.get_field(args[0], "item")),
            ("newSlot", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;") => {
                let index = mmga_runtime::unbox_primitive(ctx.rt, args[0]);
                let value = ctx.rt.new_instance(SLOT, 0);
                ctx.rt.put_field(value, "index", index);
                ctx.rt.put_field(value, "item", args[1]);
                Some(value)
            }
            ("fromSlots", "(Ljava/util/List;)Lnet/minecraft/world/item/component/ItemContainerContents;") => {
                let slots = list_values(&mut ctx, args[0])?;
                if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
                if slots.is_empty() { return Some(empty(ctx.rt)); }
                // Constructing the initial all-empty sized list computes real ItemStack
                // hashes (including AIR). Those prototypes are not fully materialized yet.
                Some(ctx.rt.throw_exception("java/lang/UnsupportedOperationException", "Nonempty container slot decoding requires ItemStack.hashStackList and canonical AIR components"))
            }
            ("asSlots", "(Lnet/minecraft/world/item/component/ItemContainerContents;)Ljava/util/List;") => {
                let list = ctx.rt.get_field(args[0], "items");
                let items = list_values(&mut ctx, list)?;
                if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
                let mut slots = Vec::new();
                for (index, item) in items.into_iter().enumerate() {
                    let empty = invoke_member(&mut ctx, op::INVOKEVIRTUAL, &MemberRef::new(STACK,"isEmpty","()Z"), &[item]);
                    if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
                    if empty.i() == 0 {
                        let slot = ctx.rt.new_instance(SLOT,0);
                        ctx.rt.put_field(slot,"index",V::I(index as i32));
                        ctx.rt.put_field(slot,"item",item);
                        slots.push(slot);
                    }
                }
                Some(ctx.rt.alloc("java/util/ArrayList",0,Data::List(slots)))
            }
            (
                "fromList",
                "(Ljava/util/List;)Lnet/minecraft/world/item/component/ItemContainerContents;",
            ) => {
                let values = list_values(&mut ctx, args[0])?;
                Some(new_from_values(&mut ctx, values, false))
            }
            (
                "items",
                "(Lnet/minecraft/world/item/component/ItemContainerContents;)Ljava/util/List;",
            ) => Some(ctx.rt.get_field(args[0], "items")),
            _ => None,
        };
    }
    if owner != CONTENTS {
        return None;
    }
    if is_static
        && name == "fromItems"
        && desc == "(Ljava/util/List;)Lnet/minecraft/world/item/component/ItemContainerContents;"
    {
        let mut values = list_values(&mut ctx, args[0])?;
        while let Some(value) = values.last() {
            let empty = invoke_member(
                &mut ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new(STACK, "isEmpty", "()Z"),
                &[*value],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            if empty.i() == 0 {
                break;
            }
            values.pop();
        }
        if values.is_empty() {
            return Some(empty(ctx.rt));
        }
        return Some(new_from_values(&mut ctx, values, true));
    }
    if is_static {
        return None;
    }
    let recv = *args.first()?;
    match (name, desc) {
        ("<init>", "(Ljava/util/List;)V") => {
            let values = list_values(&mut ctx, args[1])?;
            let value = new_from_values(&mut ctx, values, false);
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let items = ctx.rt.get_field(value, "items");
            let hash = ctx.rt.get_field(value, "hashCode");
            ctx.rt.put_field(recv, "items", items);
            ctx.rt.put_field(recv, "hashCode", hash);
            Some(V::Void)
        }
        ("hashCode", "()I") => Some(ctx.rt.get_field(recv, "hashCode")),
        ("equals", "(Ljava/lang/Object;)Z") => {
            if recv == args[1] {
                return Some(V::TRUE);
            }
            if args[1].is_null() || ctx.rt.class_of(args[1]) != Some(CONTENTS) {
                return Some(V::FALSE);
            }
            let left_items = ctx.rt.get_field(recv, "items");
            let right_items = ctx.rt.get_field(args[1], "items");
            let left = list_values(&mut ctx, left_items)?;
            let right = list_values(&mut ctx, right_items)?;
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            if left.len() != right.len() {
                return Some(V::FALSE);
            }
            for (a, b) in left.into_iter().zip(right) {
                let ae = invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new(STACK, "isEmpty", "()Z"),
                    &[a],
                );
                let be = invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new(STACK, "isEmpty", "()Z"),
                    &[b],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if ae.i() != 0 && be.i() != 0 {
                    continue;
                }
                // ItemStack.equals requires component and item registries; this host has
                // only the canonical empty path, so never turn a nonempty comparison into
                // a false success.
                return Some(ctx.rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "ItemContainerContents equality requires nonempty ItemStack equality",
                ));
            }
            Some(V::TRUE)
        }
        ("copyOne", "()Lnet/minecraft/world/item/ItemStack;") => {
            let list = ctx.rt.get_field(recv, "items");
            let items = list_values(&mut ctx, list)?;
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            Some(if let Some(first) = items.first() {
                invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new(STACK, "copy", "()Lnet/minecraft/world/item/ItemStack;"),
                    &[*first],
                )
            } else {
                crate::item_stack::get_static_field(
                    ctx.rt,
                    STACK,
                    "EMPTY",
                    "Lnet/minecraft/world/item/ItemStack;",
                )
                .unwrap()
            })
        }
        _ => None,
    }
}
fn empty(rt: &mut Rt) -> V {
    if let Some(value) = rt.get_static(CONTENTS, "EMPTY") {
        return value;
    }
    let value = rt.new_instance(CONTENTS, 0);
    let items = rt.alloc("net/minecraft/core/NonNullList", 0, Data::List(vec![]));
    rt.put_field(value, "items", items);
    rt.put_field(value, "hashCode", V::I(0));
    rt.put_static(CONTENTS, "EMPTY", value);
    value
}
fn list_values(ctx: &mut ExecCtx<'_>, list: V) -> Option<Vec<V>> {
    if list.is_null() {
        ctx.rt
            .throw_exception("java/lang/NullPointerException", "Container list is null");
        return Some(vec![]);
    }
    let count = invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new("java/util/List", "size", "()I"),
        &[list],
    )
    .i();
    if ctx.rt.pending_exception.is_some() {
        return Some(vec![]);
    }
    let mut values = Vec::new();
    for i in 0..count {
        let value = invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/util/List", "get", "(I)Ljava/lang/Object;"),
            &[list, V::I(i)],
        );
        if ctx.rt.pending_exception.is_some() {
            return Some(vec![]);
        }
        values.push(value);
    }
    Some(values)
}
fn new_from_values(ctx: &mut ExecCtx<'_>, values: Vec<V>, copy: bool) -> V {
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    if values.len() > 256 {
        return ctx.rt.throw_exception(
            "java/lang/IllegalArgumentException",
            "Got more than 256 items in ItemContainerContents",
        );
    }
    if !values.is_empty() {
        return ctx.rt.throw_exception("java/lang/UnsupportedOperationException","Nonempty ItemContainerContents construction requires ItemStack.hashStackList and canonical AIR components");
    }
    let _ = copy;
    let value = ctx.rt.new_instance(CONTENTS, 0);
    let items = ctx
        .rt
        .alloc("net/minecraft/core/NonNullList", 0, Data::List(values));
    ctx.rt.put_field(value, "items", items);
    ctx.rt.put_field(value, "hashCode", V::I(0));
    value
}
fn callback(rt: &mut Rt, name: &str, desc: &str) -> V {
    callback_as(rt, name, desc, false)
}
fn callback_as(rt: &mut Rt, name: &str, desc: &str, binary: bool) -> V {
    let interface = if binary {
        "java/util/function/BiFunction"
    } else {
        "java/util/function/Function"
    };
    rt.alloc(
        interface,
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: interface.into(),
            sam_name: "apply".into(),
            sam_desc: if binary {
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;"
            } else {
                "(Ljava/lang/Object;)Ljava/lang/Object;"
            }
            .into(),
            instantiated_desc: desc.into(),
            captures: vec![],
            target: ClosureTarget::MethodHandle {
                handle_kind: 6,
                member: MemberRef::new(CALLBACK, name, desc),
            },
        })),
    )
}

// Official 1.21.1 cxo/cxo$a: a list (max 256) of records with slot in 0..255
// and ItemStack.CODEC, mapped through fromSlots/asSlots. Empty is [] rather than {}.
fn container_codec(rt: &mut Rt) -> V {
    let linked = LinkedSpace::new();
    let mut host = mmga_runtime::NullHost;
    let index = crate::dfu::call(
        rt,
        &linked,
        &mut host,
        &MemberRef::new(CODEC, "intRange", "(II)Lcom/mojang/serialization/Codec;"),
        true,
        &[V::I(0), V::I(255)],
    )
    .unwrap();
    let index = crate::dfu::field_codec(rt, index, "slot");
    let item =
        crate::item_stack::get_static_field(rt, STACK, "CODEC", "Lcom/mojang/serialization/Codec;")
            .unwrap();
    let item = crate::dfu::field_codec(rt, item, "item");
    let index_getter = callback(
        rt,
        "slotIndex",
        "(Lnet/minecraft/world/item/component/ItemContainerContents$Slot;)Ljava/lang/Object;",
    );
    let item_getter = callback(
        rt,
        "slotItem",
        "(Lnet/minecraft/world/item/component/ItemContainerContents$Slot;)Ljava/lang/Object;",
    );
    let constructor = callback_as(
        rt,
        "newSlot",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        true,
    );
    let map = crate::dfu::record_map_codec(
        rt,
        &[(index, index_getter), (item, item_getter)],
        constructor,
    );
    let codec = crate::dfu::codec_from_map(rt, map);
    let list = crate::dfu::call(
        rt,
        &linked,
        &mut host,
        &MemberRef::new(
            CODEC,
            "sizeLimitedListOf",
            "(I)Lcom/mojang/serialization/Codec;",
        ),
        false,
        &[codec, V::I(256)],
    )
    .unwrap();
    let from = callback(
        rt,
        "fromSlots",
        "(Ljava/util/List;)Lnet/minecraft/world/item/component/ItemContainerContents;",
    );
    let to = callback(
        rt,
        "asSlots",
        "(Lnet/minecraft/world/item/component/ItemContainerContents;)Ljava/util/List;",
    );
    crate::dfu::call(rt,&linked,&mut host,&MemberRef::new(CODEC,"xmap","(Ljava/util/function/Function;Ljava/util/function/Function;)Lcom/mojang/serialization/Codec;"),false,&[list,from,to]).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn from_items_trims_empty_slots_but_nonempty_execution_remains_explicit() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ContainerHost;
        let empty_stack = crate::item_stack::get_static_field(
            &mut rt,
            STACK,
            "EMPTY",
            "Lnet/minecraft/world/item/ItemStack;",
        )
        .unwrap();
        let empties = rt.alloc(
            "java/util/List",
            0,
            Data::List(vec![empty_stack, empty_stack]),
        );
        let canonical = empty(&mut rt);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let got = invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                CONTENTS,
                "fromItems",
                "(Ljava/util/List;)Lnet/minecraft/world/item/component/ItemContainerContents;",
            ),
            &[empties],
        );
        assert_eq!(got, canonical);
        assert!(ctx.rt.pending_exception.is_none());
        let item = ctx.rt.new_instance("test/Item", 0);
        let stack = ctx.rt.new_instance(STACK, 0);
        ctx.rt.put_field(stack, "item", item);
        ctx.rt.put_field(stack, "count", V::I(1));
        let nonempty = ctx.rt.alloc("java/util/List", 0, Data::List(vec![stack]));
        invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                CONTENTS,
                "fromItems",
                "(Ljava/util/List;)Lnet/minecraft/world/item/component/ItemContainerContents;",
            ),
            &[nonempty],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
    struct ContainerHost;
    impl HostApi for ContainerHost {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            get_static_field(rt, owner, name, desc)
                .or_else(|| crate::item_stack::get_static_field(rt, owner, name, desc))
                .or_else(|| crate::codecs::get_static_field(rt, owner, name, desc))
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
            if let Some(v) = call(rt, linked, self, member, is_static, args) {
                return Some(v);
            }
            if let Some(v) = crate::item_stack::call(rt, linked, self, member, is_static, args) {
                return Some(v);
            }
            crate::codecs::call(rt, linked, self, member, is_static, args)
                .or_else(|| crate::dfu::call(rt, linked, self, member, is_static, args))
        }
    }
    fn decode_container(ctx: &mut ExecCtx<'_>, codec: V, text: &str) -> V {
        let text = ctx.rt.intern_str(text);
        let json = invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                "com/google/gson/JsonParser",
                "parseString",
                "(Ljava/lang/String;)Lcom/google/gson/JsonElement;",
            ),
            &[text],
        );
        let ops = crate::dfu::get_static_field(
            ctx.rt,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        invoke_member(ctx,op::INVOKEINTERFACE,&MemberRef::new(CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"),&[codec,ops,json])
    }
    fn result_value(ctx: &mut ExecCtx<'_>, result: V) -> V {
        invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "com/mojang/serialization/DataResult",
                "getOrThrow",
                "()Ljava/lang/Object;",
            ),
            &[result],
        )
    }
    #[test]
    fn container_json_is_a_bounded_slot_list_and_empty_roundtrips_as_array() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ContainerHost;
        let codec = get_static_field(
            &mut rt,
            CONTENTS,
            "CODEC",
            "Lcom/mojang/serialization/Codec;",
        )
        .unwrap();
        assert_eq!(
            get_static_field(
                &mut rt,
                CONTENTS,
                "CODEC",
                "Lcom/mojang/serialization/Codec;"
            ),
            Some(codec)
        );
        let empty = empty(&mut rt);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let result = decode_container(&mut ctx, codec, "[]");
        assert_eq!(result_value(&mut ctx, result), empty);
        assert!(ctx.rt.pending_exception.is_none());
        let ops = crate::dfu::get_static_field(
            ctx.rt,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let encoded = invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new(CODEC,"encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"),&[codec,ops,empty]);
        let json = result_value(&mut ctx, encoded);
        let text = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                "com/google/gson/JsonElement",
                "toString",
                "()Ljava/lang/String;",
            ),
            &[json],
        );
        assert_eq!(ctx.rt.str_of(text), Some("[]"));
        for input in [
            "{}",
            "[{\"slot\":256,\"item\":{}}]",
            "[{\"slot\":-1,\"item\":{}}]",
            "[{\"slot\":0,\"item\":{}}]",
        ] {
            let result = decode_container(&mut ctx, codec, input);
            assert!(
                ctx.rt.pending_exception.is_none(),
                "Bad JSON must remain a data error: {input}"
            );
            result_value(&mut ctx, result);
            let error = ctx.rt.take_exception().expect(input);
            assert!(ctx
                .rt
                .exception_summary(error)
                .contains("IllegalStateException"));
        }
        let too_many = format!("[{}]", vec!["{\"slot\":0,\"item\":{}}"; 257].join(","));
        let result = decode_container(&mut ctx, codec, &too_many);
        result_value(&mut ctx, result);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx.rt.exception_summary(error).contains("256"));
    }
    #[test]
    fn empty_container_stream_roundtrips_real_list_length_and_retains_bounded_child_codec() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ContainerHost;
        let codec = get_static_field(
            &mut rt,
            CONTENTS,
            "STREAM_CODEC",
            "Lnet/minecraft/network/codec/StreamCodec;",
        )
        .unwrap();
        let empty = get_static_field(
            &mut rt,
            CONTENTS,
            "EMPTY",
            "Lnet/minecraft/world/item/component/ItemContainerContents;",
        )
        .unwrap();
        let buf = rt.alloc(
            "net/minecraft/network/RegistryFriendlyByteBuf",
            0,
            Data::ByteBuf {
                bytes: vec![],
                reader_idx: 0,
            },
        );
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "encode", "(Ljava/lang/Object;Ljava/lang/Object;)V"),
            &[codec, buf, empty],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert!(matches!(&ctx.rt.get_obj(buf).unwrap().data,Data::ByteBuf{bytes,..}if bytes==&[0]));
        let decoded = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
            &[codec, buf],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.class_of(decoded), Some(CONTENTS));
        assert_eq!(ctx.rt.get_field(decoded, "hashCode"), V::I(0));
        let invalid = ctx.rt.alloc(
            "net/minecraft/network/RegistryFriendlyByteBuf",
            0,
            Data::ByteBuf {
                bytes: vec![0x81, 2],
                reader_idx: 0,
            },
        );
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
            &[codec, invalid],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx.rt.exception_summary(error).contains("maximum 256"));
    }
    #[test]
    fn empty_container_value_equality_works_through_object_owner() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ContainerHost;
        let left = empty(&mut rt);
        let values = rt.alloc("java/util/ArrayList", 0, Data::List(vec![]));
        let right = rt.new_instance(CONTENTS, 0);
        call(
            &mut rt,
            &linked,
            &mut host,
            &MemberRef::new(CONTENTS, "<init>", "(Ljava/util/List;)V"),
            false,
            &[right, values],
        )
        .unwrap();
        assert_ne!(left, right);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let equals = MemberRef::new("java/lang/Object", "equals", "(Ljava/lang/Object;)Z");
        assert_eq!(
            invoke_member(&mut ctx, op::INVOKEVIRTUAL, &equals, &[left, right]),
            V::TRUE
        );
        assert_eq!(
            invoke_member(&mut ctx, op::INVOKEVIRTUAL, &equals, &[left, V::NULL]),
            V::FALSE
        );
        let hash = MemberRef::new("java/lang/Object", "hashCode", "()I");
        assert_eq!(
            invoke_member(&mut ctx, op::INVOKEVIRTUAL, &hash, &[left]),
            V::I(0)
        );
        assert_eq!(
            invoke_member(&mut ctx, op::INVOKEVIRTUAL, &hash, &[right]),
            V::I(0)
        );
        assert!(ctx.rt.pending_exception.is_none());
    }
}
