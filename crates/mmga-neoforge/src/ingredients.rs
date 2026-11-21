//! Ingredient metadata from Minecraft 1.21.1 `cyw` and NeoForge's Ingredient patch.
//! Official client/mappings are the SHA-1 verified artifacts recorded in compatibility.md.
//! NeoForge patch inspected at 1.21.1 commit 89e887b367ab0c11cb58ce8500920efadc17b803.
//! Tag values retain their TagKey; tag membership is never guessed from missing data.

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, Data, ExecCtx, Rt, V};

const INGREDIENT: &str = "net/minecraft/world/item/crafting/Ingredient";
const TAG_VALUE: &str = "net/minecraft/world/item/crafting/Ingredient$TagValue";
const ITEM_VALUE: &str = "net/minecraft/world/item/crafting/Ingredient$ItemValue";
const VALUE: &str = "net/minecraft/world/item/crafting/Ingredient$Value";
const STACK: &str = "net/minecraft/world/item/ItemStack";

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner == INGREDIENT
        && name == "EMPTY"
        && desc == "Lnet/minecraft/world/item/crafting/Ingredient;"
    {
        return Some(empty(rt));
    }
    None
}
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if is_static {
        if owner != INGREDIENT {
            return None;
        }
        return match (name,desc) {
            ("of","()Lnet/minecraft/world/item/crafting/Ingredient;") => Some(empty(ctx.rt)),
            ("fromValues","(Ljava/util/stream/Stream;)Lnet/minecraft/world/item/crafting/Ingredient;") => {
                let values=match stream_values(ctx,*args.first()?) { Ok(values)=>values,Err(error)=>return Some(error) };
                if values.iter().any(|value| !value.is_null() && !ctx.linked.is_subtype(ctx.rt.class_of(*value).unwrap_or(""),VALUE,ctx.host)) {
                    return Some(ctx.rt.throw_exception("java/lang/ArrayStoreException","Ingredient.Value[] received a non-value"));
                }
                Some(from_values(ctx.rt,values))
            }
            ("of","(Lnet/minecraft/tags/TagKey;)Lnet/minecraft/world/item/crafting/Ingredient;") => {
                let value=ctx.rt.new_instance(TAG_VALUE,0);
                ctx.rt.put_field(value,"tag",*args.first()?);
                Some(from_values(ctx.rt,vec![value]))
            }
            ("of","([Lnet/minecraft/world/item/ItemStack;)Lnet/minecraft/world/item/crafting/Ingredient;") => {
                let items=match array_values(ctx,*args.first()?) {Ok(items)=>items,Err(error)=>return Some(error)};
                Some(from_stacks(ctx,items))
            }
            ("of","(Ljava/util/stream/Stream;)Lnet/minecraft/world/item/crafting/Ingredient;") => {
                let items=match stream_values(ctx,*args.first()?) {Ok(items)=>items,Err(error)=>return Some(error)};
                Some(from_stacks(ctx,items))
            }
            ("of","([Lnet/minecraft/world/level/ItemLike;)Lnet/minecraft/world/item/crafting/Ingredient;") => {
                let items=match array_values(ctx,*args.first()?) {Ok(items)=>items,Err(error)=>return Some(error)};
                let mut stacks=Vec::new();
                for item in items {
                    let stack=ctx.rt.new_instance(STACK,0);
                    invoke_member(ctx,op::INVOKESPECIAL,&MemberRef::new(STACK,"<init>","(Lnet/minecraft/world/level/ItemLike;)V"),&[stack,item]);
                    if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
                    stacks.push(stack);
                }
                Some(from_stacks(ctx,stacks))
            }
            _=>None,
        };
    }
    let recv = *args.first()?;
    let value_owner = if owner == VALUE {
        ctx.rt.class_of(recv)?.to_owned()
    } else {
        owner.to_owned()
    };
    if matches!(value_owner.as_str(), TAG_VALUE | ITEM_VALUE) {
        return match (value_owner.as_str(), name, desc) {
            (TAG_VALUE, "<init>", "(Lnet/minecraft/tags/TagKey;)V") => {
                ctx.rt.put_field(recv, "tag", *args.get(1)?);
                Some(V::Void)
            }
            (ITEM_VALUE, "<init>", "(Lnet/minecraft/world/item/ItemStack;)V") => {
                ctx.rt.put_field(recv, "item", *args.get(1)?);
                Some(V::Void)
            }
            (TAG_VALUE, "tag", "()Lnet/minecraft/tags/TagKey;") => {
                Some(ctx.rt.get_field(recv, "tag"))
            }
            (ITEM_VALUE, "item", "()Lnet/minecraft/world/item/ItemStack;") => {
                Some(ctx.rt.get_field(recv, "item"))
            }
            (_, "equals", "(Ljava/lang/Object;)Z") => Some(value_equals(ctx, recv, *args.get(1)?)),
            (ITEM_VALUE, "hashCode", "()I") => {
                let stack = ctx.rt.get_field(recv, "item");
                let item = stack_call(ctx, stack, "getItem", "()Lnet/minecraft/world/item/Item;");
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let hash = invoke_member(
                    ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new("java/lang/Object", "hashCode", "()I"),
                    &[item],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let count = stack_call(ctx, stack, "getCount", "()I");
                Some(V::I(hash.i().wrapping_mul(31).wrapping_add(count.i())))
            }
            (TAG_VALUE, "hashCode", "()I") => Some(unsupported(
                ctx.rt,
                "Ingredient.TagValue record hash for TagKey",
            )),
            (_, "toString", "()Ljava/lang/String;") => Some(unsupported(
                ctx.rt,
                "Ingredient.Value record string representation",
            )),
            (TAG_VALUE, "getItems", "()Ljava/util/Collection;") => Some(unsupported(
                ctx.rt,
                "Ingredient tag membership and loaded empty-tag placeholders",
            )),
            (ITEM_VALUE, "getItems", "()Ljava/util/Collection;") => {
                // Collections.singleton is immutable; the existing general Set
                // host is mutable, so retain the value and reject this view until
                // a faithful singleton collection implementation is available.
                Some(unsupported(
                    ctx.rt,
                    "immutable Ingredient.ItemValue singleton collection",
                ))
            }
            _ => None,
        };
    }
    if owner != INGREDIENT
        && !(owner == "java/util/function/Predicate" && ctx.rt.class_of(recv) == Some(INGREDIENT))
    {
        return None;
    }
    match (name, desc) {
        ("<init>", "([Lnet/minecraft/world/item/crafting/Ingredient$Value;)V") => {
            initialize(ctx.rt, recv, *args.get(1)?);
            Some(V::Void)
        }
        ("<init>", "(Ljava/util/stream/Stream;)V") => {
            let values = match stream_values(ctx, *args.get(1)?) {
                Ok(values) => values,
                Err(error) => return Some(error),
            };
            let array = ctx.rt.alloc(
                "[Lnet/minecraft/world/item/crafting/Ingredient$Value;",
                0,
                Data::Array(values),
            );
            initialize(ctx.rt, recv, array);
            Some(V::Void)
        }
        ("isCustom", "()Z") => Some(V::bool(
            !ctx.rt.get_field(recv, "customIngredient").is_null(),
        )),
        ("getCustomIngredient", "()Lnet/neoforged/neoforge/common/crafting/ICustomIngredient;") => {
            Some(ctx.rt.get_field(recv, "customIngredient"))
        }
        ("isSimple", "()Z") => {
            if ctx.rt.get_field(recv, "customIngredient").is_null() {
                Some(V::TRUE)
            } else {
                Some(unsupported(ctx.rt, "custom Ingredient isSimple callback"))
            }
        }
        ("isEmpty", "()Z") => {
            let values = ctx.rt.get_field(recv, "values");
            let values = match array_values(ctx, values) {
                Ok(values) => values,
                Err(error) => return Some(error),
            };
            Some(V::bool(
                values.is_empty() && ctx.rt.get_field(recv, "customIngredient").is_null(),
            ))
        }
        ("getValues", "()[Lnet/minecraft/world/item/crafting/Ingredient$Value;") => {
            if !ctx.rt.get_field(recv, "customIngredient").is_null() {
                return Some(ctx.rt.throw_exception(
                    "java/lang/IllegalStateException",
                    "Cannot retrieve values from custom ingredient!",
                ));
            }
            Some(ctx.rt.get_field(recv, "values"))
        }
        ("getItems", "()[Lnet/minecraft/world/item/ItemStack;") => Some(get_items(ctx, recv)),
        ("hasNoItems", "()Z") => {
            let items = get_items(ctx, recv);
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            Some(V::bool(
                matches!(ctx.rt.get_obj(items).map(|object|&object.data),Some(Data::Array(items)) if items.is_empty()),
            ))
        }
        ("test", "(Lnet/minecraft/world/item/ItemStack;)Z" | "(Ljava/lang/Object;)Z") => {
            let stack = *args.get(1)?;
            if stack.is_null() {
                return Some(V::FALSE);
            }
            if ctx.rt.class_of(stack) != Some(STACK) {
                return Some(ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    "Ingredient.test requires ItemStack",
                ));
            }
            if !ctx.rt.get_field(recv, "customIngredient").is_null() {
                return Some(unsupported(ctx.rt, "custom Ingredient predicate"));
            }
            let values = match array_values(ctx, ctx.rt.get_field(recv, "values")) {
                Ok(values) => values,
                Err(error) => return Some(error),
            };
            if values.is_empty() {
                return Some(stack_call(ctx, stack, "isEmpty", "()Z"));
            }
            let items = get_items(ctx, recv);
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let candidates = match array_values(ctx, items) {
                Ok(values) => values,
                Err(error) => return Some(error),
            };
            let item = stack_call(ctx, stack, "getItem", "()Lnet/minecraft/world/item/Item;");
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            for candidate in candidates {
                let current = stack_call(
                    ctx,
                    candidate,
                    "getItem",
                    "()Lnet/minecraft/world/item/Item;",
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if current == item {
                    return Some(V::TRUE);
                }
            }
            Some(V::FALSE)
        }
        ("equals", "(Ljava/lang/Object;)Z") => {
            let other = *args.get(1)?;
            if ctx.rt.class_of(other) != Some(INGREDIENT) {
                return Some(V::FALSE);
            }
            if !ctx.rt.get_field(recv, "customIngredient").is_null()
                || !ctx.rt.get_field(other, "customIngredient").is_null()
            {
                return Some(unsupported(ctx.rt, "custom Ingredient equality"));
            }
            let left = match array_values(ctx, ctx.rt.get_field(recv, "values")) {
                Ok(values) => values,
                Err(error) => return Some(error),
            };
            let right = match array_values(ctx, ctx.rt.get_field(other, "values")) {
                Ok(values) => values,
                Err(error) => return Some(error),
            };
            if left.len() != right.len() {
                return Some(V::FALSE);
            }
            for (a, b) in left.into_iter().zip(right) {
                if a == b {
                    continue;
                }
                let equal = value_equals(ctx, a, b);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if equal.i() == 0 {
                    return Some(V::FALSE);
                }
            }
            Some(V::TRUE)
        }
        ("hashCode", "()I") => {
            if !ctx.rt.get_field(recv, "customIngredient").is_null() {
                return Some(unsupported(ctx.rt, "custom Ingredient hash"));
            }
            let array = ctx.rt.get_field(recv, "values");
            if array.is_null() {
                return Some(V::I(0));
            }
            let values = match array_values(ctx, array) {
                Ok(values) => values,
                Err(error) => return Some(error),
            };
            let mut hash = 1i32;
            for value in values {
                let value_hash = if value.is_null() {
                    V::I(0)
                } else {
                    invoke_member(
                        ctx,
                        op::INVOKEVIRTUAL,
                        &MemberRef::new("java/lang/Object", "hashCode", "()I"),
                        &[value],
                    )
                };
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                hash = hash.wrapping_mul(31).wrapping_add(value_hash.i());
            }
            Some(V::I(hash))
        }
        _ => None,
    }
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == INGREDIENT && sup == "java/util/function/Predicate")
        || (matches!(sub, TAG_VALUE | ITEM_VALUE) && matches!(sup, VALUE | "java/lang/Record"))
}

fn empty(rt: &mut Rt) -> V {
    if let Some(value) = rt.get_static(INGREDIENT, "EMPTY") {
        return value;
    }
    let ingredient = rt.new_instance(INGREDIENT, 0);
    let array = rt.alloc(
        "[Lnet/minecraft/world/item/crafting/Ingredient$Value;",
        0,
        Data::Array(Vec::new()),
    );
    initialize(rt, ingredient, array);
    rt.put_static(INGREDIENT, "EMPTY", ingredient);
    ingredient
}
fn initialize(rt: &mut Rt, ingredient: V, values: V) {
    for (name, value) in [
        ("values", values),
        ("itemStacks", V::NULL),
        ("stackingIds", V::NULL),
        ("customIngredient", V::NULL),
    ] {
        rt.put_field(ingredient, name, value);
    }
}
fn from_values(rt: &mut Rt, values: Vec<V>) -> V {
    if values.is_empty() {
        return empty(rt);
    }
    let ingredient = rt.new_instance(INGREDIENT, 0);
    let array = rt.alloc(
        "[Lnet/minecraft/world/item/crafting/Ingredient$Value;",
        0,
        Data::Array(values),
    );
    initialize(rt, ingredient, array);
    ingredient
}
fn array_values(ctx: &mut ExecCtx<'_>, array: V) -> Result<Vec<V>, V> {
    if array.is_null() {
        return Err(ctx
            .rt
            .throw_exception("java/lang/NullPointerException", "Ingredient array is null"));
    }
    match ctx.rt.get_obj(array).map(|object| &object.data) {
        Some(Data::Array(values)) => Ok(values.clone()),
        _ => Err(unsupported(ctx.rt, "Ingredient input array representation")),
    }
}
fn stream_values(ctx: &mut ExecCtx<'_>, stream: V) -> Result<Vec<V>, V> {
    if stream.is_null() {
        return Err(ctx.rt.throw_exception(
            "java/lang/NullPointerException",
            "Ingredient stream is null",
        ));
    }
    // Data::Stream stores values after actual host map/filter callbacks execute.
    // Unknown stream implementations require their real traversal API.
    match ctx.rt.get_obj(stream).map(|object| &object.data) {
        Some(Data::Stream(values)) => Ok(values.clone()),
        _ => Err(unsupported(ctx.rt, "Ingredient input Stream traversal")),
    }
}
fn from_stacks(ctx: &mut ExecCtx<'_>, stacks: Vec<V>) -> V {
    let mut values = Vec::new();
    for stack in stacks {
        let is_empty = stack_call(ctx, stack, "isEmpty", "()Z");
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
        if is_empty.i() == 0 {
            let value = ctx.rt.new_instance(ITEM_VALUE, 0);
            ctx.rt.put_field(value, "item", stack);
            values.push(value);
        }
    }
    from_values(ctx.rt, values)
}
fn stack_call(ctx: &mut ExecCtx<'_>, stack: V, name: &str, desc: &str) -> V {
    invoke_member(
        ctx,
        op::INVOKEVIRTUAL,
        &MemberRef::new(STACK, name, desc),
        &[stack],
    )
}
fn get_items(ctx: &mut ExecCtx<'_>, ingredient: V) -> V {
    let cached = ctx.rt.get_field(ingredient, "itemStacks");
    if !cached.is_null() {
        return cached;
    }
    if !ctx.rt.get_field(ingredient, "customIngredient").is_null() {
        return unsupported(ctx.rt, "custom Ingredient item enumeration");
    }
    let values = match array_values(ctx, ctx.rt.get_field(ingredient, "values")) {
        Ok(values) => values,
        Err(error) => return error,
    };
    if !values.is_empty() {
        // NeoForge requires real type+component equality during deduplication,
        // and real loaded tag membership (with its empty-tag placeholder).
        return unsupported(
            ctx.rt,
            "Ingredient item/component deduplication or tag membership",
        );
    }
    let items = ctx.rt.alloc(
        "[Lnet/minecraft/world/item/ItemStack;",
        0,
        Data::Array(Vec::new()),
    );
    ctx.rt.put_field(ingredient, "itemStacks", items);
    items
}
fn value_equals(ctx: &mut ExecCtx<'_>, left: V, right: V) -> V {
    if left.is_null() {
        return V::bool(right.is_null());
    }
    let class = ctx.rt.class_of(left).unwrap_or("").to_owned();
    if ctx.rt.class_of(right) != Some(class.as_str()) {
        return V::FALSE;
    }
    if class == TAG_VALUE {
        let a = ctx.rt.get_field(left, "tag");
        let b = ctx.rt.get_field(right, "tag");
        if a.is_null() || b.is_null() {
            return ctx.rt.throw_exception(
                "java/lang/NullPointerException",
                "Ingredient.TagValue tag is null",
            );
        }
        let location = |rt: &Rt, value: V| {
            rt.handle_of(value)
                .filter(|(kind, ..)| *kind == "tag_key")
                .and_then(|(_, _, tag, _)| {
                    tag.split_once('|').map(|(_, location)| location.to_owned())
                })
        };
        match (location(ctx.rt, a), location(ctx.rt, b)) {
            (Some(a), Some(b)) => V::bool(a == b),
            _ => unsupported(ctx.rt, "Ingredient.TagValue tag location equality"),
        }
    } else if class == ITEM_VALUE {
        let a = ctx.rt.get_field(left, "item");
        let b = ctx.rt.get_field(right, "item");
        let a_item = stack_call(ctx, a, "getItem", "()Lnet/minecraft/world/item/Item;");
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
        let b_item = stack_call(ctx, b, "getItem", "()Lnet/minecraft/world/item/Item;");
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
        if a_item != b_item {
            return V::FALSE;
        }
        let a_count = stack_call(ctx, a, "getCount", "()I");
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
        let b_count = stack_call(ctx, b, "getCount", "()I");
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
        V::bool(a_count == b_count)
    } else {
        unsupported(ctx.rt, "unknown Ingredient.Value equality")
    }
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
    use mmga_runtime::{invoke_member, read_static_field, HostApi, LinkedSpace};

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
                .or_else(|| crate::item_stack::get_static_field(rt, owner, name, desc))
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
            .or_else(|| crate::item_stack::call(rt, linked, self, member, is_static, args))
        }
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            super::is_subtype(sub, sup)
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
    #[test]
    fn empty_ingredient_is_canonical_but_an_unresolved_tag_is_not_empty() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let empty = read_static_field(
            &mut ctx,
            INGREDIENT,
            "EMPTY",
            "Lnet/minecraft/world/item/crafting/Ingredient;",
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "Ingredient.EMPTY unavailable"
        );
        assert_eq!(
            invoke(&mut ctx, INGREDIENT, "isEmpty", "()Z", false, &[empty]),
            V::TRUE
        );
        let stream = ctx
            .rt
            .alloc("java/util/stream/Stream", 0, Data::Stream(Vec::new()));
        assert_eq!(
            invoke(
                &mut ctx,
                INGREDIENT,
                "fromValues",
                "(Ljava/util/stream/Stream;)Lnet/minecraft/world/item/crafting/Ingredient;",
                true,
                &[stream]
            ),
            empty
        );
        assert_eq!(
            invoke(
                &mut ctx,
                INGREDIENT,
                "test",
                "(Lnet/minecraft/world/item/ItemStack;)Z",
                false,
                &[empty, V::NULL]
            ),
            V::FALSE
        );
        let stack = read_static_field(
            &mut ctx,
            STACK,
            "EMPTY",
            "Lnet/minecraft/world/item/ItemStack;",
        );
        assert_eq!(
            invoke(
                &mut ctx,
                INGREDIENT,
                "test",
                "(Lnet/minecraft/world/item/ItemStack;)Z",
                false,
                &[empty, stack]
            ),
            V::TRUE
        );
        let tag = ctx.rt.new_pinned_handle(
            "net/minecraft/tags/TagKey",
            "tag_key",
            0,
            "minecraft:item|c:nuggets/iron",
        );
        let value = ctx.rt.new_instance(TAG_VALUE, 0);
        invoke(
            &mut ctx,
            TAG_VALUE,
            "<init>",
            "(Lnet/minecraft/tags/TagKey;)V",
            false,
            &[value, tag],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                TAG_VALUE,
                "tag",
                "()Lnet/minecraft/tags/TagKey;",
                false,
                &[value]
            ),
            tag
        );
        let stream = ctx
            .rt
            .alloc("java/util/stream/Stream", 0, Data::Stream(vec![value]));
        let ingredient = invoke(
            &mut ctx,
            INGREDIENT,
            "fromValues",
            "(Ljava/util/stream/Stream;)Lnet/minecraft/world/item/crafting/Ingredient;",
            true,
            &[stream],
        );
        assert_eq!(
            invoke(&mut ctx, INGREDIENT, "isEmpty", "()Z", false, &[ingredient]),
            V::FALSE
        );
        invoke(
            &mut ctx,
            INGREDIENT,
            "getItems",
            "()[Lnet/minecraft/world/item/ItemStack;",
            false,
            &[ingredient],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
    }

    #[test]
    fn values_preserve_order_original_references_and_backing_array_mutation() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let mut values = Vec::new();
        for label in [
            "minecraft:item|c:nuggets/iron",
            "minecraft:item|c:nuggets/zinc",
        ] {
            let tag = ctx
                .rt
                .new_pinned_handle("net/minecraft/tags/TagKey", "tag_key", 0, label);
            let value = ctx.rt.new_instance(TAG_VALUE, 0);
            invoke(
                &mut ctx,
                TAG_VALUE,
                "<init>",
                "(Lnet/minecraft/tags/TagKey;)V",
                false,
                &[value, tag],
            );
            values.push(value);
        }
        let stream = ctx
            .rt
            .alloc("java/util/stream/Stream", 0, Data::Stream(values.clone()));
        let ingredient = invoke(
            &mut ctx,
            INGREDIENT,
            "fromValues",
            "(Ljava/util/stream/Stream;)Lnet/minecraft/world/item/crafting/Ingredient;",
            true,
            &[stream],
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "fromValues must preserve real TagValue metadata"
        );
        let backing = invoke(
            &mut ctx,
            INGREDIENT,
            "getValues",
            "()[Lnet/minecraft/world/item/crafting/Ingredient$Value;",
            false,
            &[ingredient],
        );
        assert!(
            matches!(&ctx.rt.get_obj(backing).unwrap().data,Data::Array(array) if array==&values)
        );
        if let Data::Array(array) = &mut ctx.rt.get_obj_mut(backing).unwrap().data {
            array[0] = values[1];
        }
        let same = invoke(
            &mut ctx,
            INGREDIENT,
            "getValues",
            "()[Lnet/minecraft/world/item/crafting/Ingredient$Value;",
            false,
            &[ingredient],
        );
        assert_eq!(same, backing);
        assert!(
            matches!(&ctx.rt.get_obj(same).unwrap().data,Data::Array(array) if array[0]==values[1])
        );
        let equivalent_tag = ctx.rt.new_pinned_handle(
            "net/minecraft/tags/TagKey",
            "tag_key",
            0,
            "minecraft:item|c:nuggets/zinc",
        );
        let equivalent_value = ctx.rt.new_instance(TAG_VALUE, 0);
        invoke(
            &mut ctx,
            TAG_VALUE,
            "<init>",
            "(Lnet/minecraft/tags/TagKey;)V",
            false,
            &[equivalent_value, equivalent_tag],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                TAG_VALUE,
                "equals",
                "(Ljava/lang/Object;)Z",
                false,
                &[values[1], equivalent_value]
            ),
            V::TRUE
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn stack_values_keep_original_stack_and_item_count_equality_and_propagate_null() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let item = ctx.rt.new_instance("net/minecraft/world/item/Item", 0);
        let mut stacks = Vec::new();
        for count in [1, 1, 2] {
            let stack = ctx.rt.new_instance(STACK, 0);
            ctx.rt.put_field(stack, "item", item);
            ctx.rt.put_field(stack, "count", V::I(count));
            stacks.push(stack);
        }
        let empty_stack = read_static_field(
            &mut ctx,
            STACK,
            "EMPTY",
            "Lnet/minecraft/world/item/ItemStack;",
        );
        let input = ctx.rt.alloc(
            "[Lnet/minecraft/world/item/ItemStack;",
            0,
            Data::Array(vec![stacks[0], empty_stack]),
        );
        let ingredient = invoke(
            &mut ctx,
            INGREDIENT,
            "of",
            "([Lnet/minecraft/world/item/ItemStack;)Lnet/minecraft/world/item/crafting/Ingredient;",
            true,
            &[input],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let values = invoke(
            &mut ctx,
            INGREDIENT,
            "getValues",
            "()[Lnet/minecraft/world/item/crafting/Ingredient$Value;",
            false,
            &[ingredient],
        );
        let first = match &ctx.rt.get_obj(values).unwrap().data {
            Data::Array(values) => {
                assert_eq!(values.len(), 1);
                values[0]
            }
            _ => panic!("Value[]"),
        };
        assert_eq!(
            invoke(
                &mut ctx,
                ITEM_VALUE,
                "item",
                "()Lnet/minecraft/world/item/ItemStack;",
                false,
                &[first]
            ),
            stacks[0]
        );
        let mut records = Vec::new();
        for stack in &stacks[1..] {
            let record = ctx.rt.new_instance(ITEM_VALUE, 0);
            invoke(
                &mut ctx,
                ITEM_VALUE,
                "<init>",
                "(Lnet/minecraft/world/item/ItemStack;)V",
                false,
                &[record, *stack],
            );
            records.push(record);
        }
        assert_eq!(
            invoke(
                &mut ctx,
                ITEM_VALUE,
                "equals",
                "(Ljava/lang/Object;)Z",
                false,
                &[first, records[0]]
            ),
            V::TRUE
        );
        assert_eq!(
            invoke(
                &mut ctx,
                ITEM_VALUE,
                "equals",
                "(Ljava/lang/Object;)Z",
                false,
                &[first, records[1]]
            ),
            V::FALSE
        );
        let hashes = records
            .iter()
            .map(|record| invoke(&mut ctx, ITEM_VALUE, "hashCode", "()I", false, &[*record]))
            .collect::<Vec<_>>();
        assert_eq!(hashes[1].i(), hashes[0].i().wrapping_add(1));
        let null_input = ctx.rt.alloc(
            "[Lnet/minecraft/world/item/ItemStack;",
            0,
            Data::Array(vec![V::NULL]),
        );
        invoke(
            &mut ctx,
            INGREDIENT,
            "of",
            "([Lnet/minecraft/world/item/ItemStack;)Lnet/minecraft/world/item/crafting/Ingredient;",
            true,
            &[null_input],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/NullPointerException")
        );
    }
}
