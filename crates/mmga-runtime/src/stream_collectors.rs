//! Joining collectors from Java 21 Collectors (blob 54d1d4acd732d3cdda59fec9edfdcecff998c27c).
//! Captured CharSequence arguments stay live until the StringJoiner supplier runs.
//! Unsupported collectors fail before the legacy generic collection fallback.

use crate::{invoke_member, Data, ExecCtx, V};
use mmga_classfile::{op, MemberRef};
const COLLECTOR: &str = "java/util/stream/Collectors$CollectorImpl";
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static && member.owner == "java/util/stream/Collectors" && member.name == "joining" {
        let (plain,delimiter,prefix,suffix)=match member.desc.as_str(){
            "()Ljava/util/stream/Collector;"=>(true,V::NULL,V::NULL,V::NULL),
            "(Ljava/lang/CharSequence;)Ljava/util/stream/Collector;"=>{
                let empty=ctx.rt.intern_str("");(false,*args.first()?,empty,empty)
            }
            "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Ljava/util/stream/Collector;"=>(false,*args.first()?,*args.get(1)?,*args.get(2)?),
            _=>return None,
        };
        let collector = ctx.rt.new_instance(COLLECTOR, 0);
        ctx.rt.put_field(collector, "$joining", V::TRUE);
        ctx.rt.put_field(collector, "$plain", V::bool(plain));
        ctx.rt.put_field(collector, "$delimiter", delimiter);
        ctx.rt.put_field(collector, "$prefix", prefix);
        ctx.rt.put_field(collector, "$suffix", suffix);
        return Some(collector);
    }
    if is_static || member.owner != "java/util/stream/Stream" {
        return None;
    }
    let stream = *args.first()?;
    if ctx.rt.get_field(stream, "$terminalConsumed").i() != 0 {
        return Some(ctx.rt.throw_exception(
            "java/lang/IllegalStateException",
            "stream has already been operated upon or closed",
        ));
    }
    if member.name != "collect" {
        return None;
    }
    let Some(Data::Stream(items)) = ctx.rt.get_obj(stream).map(|object| &object.data) else {
        return None;
    };
    let items = items.clone();
    if member.desc != "(Ljava/util/stream/Collector;)Ljava/lang/Object;" {
        return Some(unsupported(ctx, "Stream.collect function overload"));
    }
    let collector = *args.get(1)?;
    if collector.is_null() {
        return Some(
            ctx.rt
                .throw_exception("java/lang/NullPointerException", "Stream.collect collector"),
        );
    }
    if ctx.rt.get_field(collector, "$joining") != V::TRUE {
        if ctx
            .rt
            .handle_of(collector)
            .is_some_and(|(kind, id, tag, _)| {
                kind == "collector" && matches!((id, tag), (1, "toList") | (2, "toSet"))
            })
        {
            return None;
        }
        mark_consumed(ctx, stream);
        return Some(unsupported(ctx, "unknown Collector execution"));
    }
    // BaseStream consumes the pipeline before the collector supplier runs;
    // failure during supplier/accumulation does not make this stream reusable.
    mark_consumed(ctx, stream);
    let plain = ctx.rt.get_field(collector, "$plain").i() != 0;
    let (delimiter, prefix, suffix) = if plain {
        (String::new(), String::new(), String::new())
    } else {
        let prefix = ctx.rt.get_field(collector, "$prefix");
        let delimiter = ctx.rt.get_field(collector, "$delimiter");
        let suffix = ctx.rt.get_field(collector, "$suffix");
        // StringJoiner validates all three references before taking any toString snapshot.
        for (value, name) in [
            (prefix, "prefix"),
            (delimiter, "delimiter"),
            (suffix, "suffix"),
        ] {
            if value.is_null() {
                return Some(ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    &format!("StringJoiner {name}"),
                ));
            }
        }
        let prefix = match sequence_string(ctx, prefix, false) {
            Ok(value) => value,
            Err(error) => return Some(error),
        };
        let delimiter = match sequence_string(ctx, delimiter, false) {
            Ok(value) => value,
            Err(error) => return Some(error),
        };
        let suffix = match sequence_string(ctx, suffix, false) {
            Ok(value) => value,
            Err(error) => return Some(error),
        };
        (delimiter, prefix, suffix)
    };
    let mut output = prefix;
    for (index, item) in items.into_iter().enumerate() {
        if index != 0 {
            output.push_str(&delimiter);
        }
        let text = match sequence_string(ctx, item, plain) {
            Ok(value) => value,
            Err(error) => return Some(error),
        };
        output.push_str(&text);
    }
    output.push_str(&suffix);
    Some(ctx.rt.new_str(output))
}
fn sequence_string(ctx: &mut ExecCtx<'_>, value: V, plain_append: bool) -> Result<String, V> {
    if value.is_null() {
        return Ok("null".into());
    }
    let class = ctx.rt.class_of(value).unwrap_or("");
    if !ctx
        .linked
        .is_subtype(class, "java/lang/CharSequence", ctx.host)
    {
        return Err(ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "Joining collector requires CharSequence elements",
        ));
    }
    if plain_append {
        if let Some(Data::Str(text) | Data::StringBuilder(text)) =
            ctx.rt.get_obj(value).map(|object| &object.data)
        {
            return Ok(text.clone());
        }
        // StringBuilder.append(CharSequence) traverses the actual sequence; unlike
        // StringJoiner.add it must not substitute a user-defined toString value.
        let length = invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/lang/CharSequence", "length", "()I"),
            &[value],
        );
        if ctx.rt.pending_exception.is_some() {
            return Err(V::NULL);
        }
        if length.i() < 0 {
            return Err(ctx.rt.throw_exception(
                "java/lang/IndexOutOfBoundsException",
                "negative CharSequence length",
            ));
        }
        let mut units = Vec::new();
        for index in 0..length.i() {
            let unit = invoke_member(
                ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new("java/lang/CharSequence", "charAt", "(I)C"),
                &[value, V::I(index)],
            );
            if ctx.rt.pending_exception.is_some() {
                return Err(V::NULL);
            }
            units.push(unit.i() as u16);
        }
        return String::from_utf16(&units).map_err(|_| {
            unsupported(
                ctx,
                "lone UTF16 surrogate retention in StringBuilder joining",
            )
        });
    }
    let text = invoke_member(
        ctx,
        op::INVOKEVIRTUAL,
        &MemberRef::new("java/lang/Object", "toString", "()Ljava/lang/String;"),
        &[value],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err(V::NULL);
    }
    if text.is_null() {
        return Err(ctx.rt.throw_exception(
            "java/lang/NullPointerException",
            "CharSequence.toString returned null",
        ));
    }
    ctx.rt.str_of(text).map(str::to_owned).ok_or_else(|| {
        ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "CharSequence.toString must return String",
        )
    })
}
fn mark_consumed(ctx: &mut ExecCtx<'_>, stream: V) {
    // Terminal evaluation already copied the captured elements. Replace the
    // consumable payload with state rather than pinning a global stream cache.
    if let Some(object) = ctx.rt.get_obj_mut(stream) {
        object.data = Data::Fields(crate::FxHashMap::default());
    }
    ctx.rt.put_field(stream, "$terminalConsumed", V::TRUE);
}
fn unsupported(ctx: &mut ExecCtx<'_>, operation: &str) -> V {
    ctx.rt.throw_exception(
        "java/lang/UnsupportedOperationException",
        &format!("MMGA does not implement {operation}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{invoke_member, LinkedSpace, NativeClassDef, NativeModuleRegistry, NullHost, Rt};
    use mmga_classfile::op;
    fn joining(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let desc=match args.len(){0=>"()Ljava/util/stream/Collector;",1=>"(Ljava/lang/CharSequence;)Ljava/util/stream/Collector;",_=>"(Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Ljava/util/stream/Collector;"};
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new("java/util/stream/Collectors", "joining", desc),
            args,
        )
    }
    fn collect(ctx: &mut ExecCtx<'_>, values: Vec<V>, collector: V) -> V {
        let stream = ctx
            .rt
            .alloc("java/util/stream/Stream", 0, Data::Stream(values));
        collect_stream(ctx, stream, collector)
    }
    fn collect_stream(ctx: &mut ExecCtx<'_>, stream: V, collector: V) -> V {
        invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "java/util/stream/Stream",
                "collect",
                "(Ljava/util/stream/Collector;)Ljava/lang/Object;",
            ),
            &[stream, collector],
        )
    }
    #[test]
    fn joining_returns_strings_in_encounter_order_with_unicode_nulls_and_empty_prefix() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let delim = ctx.rt.new_str(" ");
        let words = vec![ctx.rt.new_str("Brass"), ctx.rt.new_str("Ingot")];
        let collector = joining(&mut ctx, &[delim]);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "joining collector missing"
        );
        let result = collect(&mut ctx, words, collector);
        assert_eq!(ctx.rt.str_of(result), Some("Brass Ingot"));
        let delim = ctx.rt.new_str("・");
        let prefix = ctx.rt.new_str("[");
        let suffix = ctx.rt.new_str("]");
        let collector = joining(&mut ctx, &[delim, prefix, suffix]);
        let values = vec![ctx.rt.new_str("铜"), V::NULL, ctx.rt.new_str("齿轮")];
        let result = collect(&mut ctx, values, collector);
        assert_eq!(ctx.rt.str_of(result), Some("[铜・null・齿轮]"));
        let result = collect(&mut ctx, Vec::new(), collector);
        assert_eq!(ctx.rt.str_of(result), Some("[]"));
        let plain = joining(&mut ctx, &[]);
        let values = vec![ctx.rt.new_str("𐐀"), ctx.rt.new_str("😀")];
        let result = collect(&mut ctx, values, plain);
        assert_eq!(ctx.rt.str_of(result), Some("𐐀😀"));
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn captures_snapshot_at_collection_and_nulls_fail_when_supplier_runs() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let delim = ctx.rt.alloc(
            "java/lang/StringBuilder",
            0,
            Data::StringBuilder("|".into()),
        );
        let collector = joining(&mut ctx, &[delim]);
        assert!(ctx.rt.pending_exception.is_none());
        let append = ctx.rt.new_str(":");
        invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                "java/lang/StringBuilder",
                "append",
                "(Ljava/lang/String;)Ljava/lang/StringBuilder;",
            ),
            &[delim, append],
        );
        let values = vec![ctx.rt.new_str("a"), ctx.rt.new_str("b")];
        let result = collect(&mut ctx, values, collector);
        assert_eq!(ctx.rt.str_of(result), Some("a|:b"));
        let null_delim = joining(&mut ctx, &[V::NULL]);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "joining(null) must defer StringJoiner construction"
        );
        collect(&mut ctx, Vec::new(), null_delim);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/NullPointerException")
        );
        let unknown = ctx.rt.new_instance("test/UnknownCollector", 0);
        collect(&mut ctx, Vec::new(), unknown);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
    }
    #[test]
    fn terminal_collect_cannot_repeat_after_success_or_supplier_failure() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let good = joining(&mut ctx, &[]);
        let bad = joining(&mut ctx, &[V::NULL]);
        for collector in [good, bad] {
            let stream = ctx
                .rt
                .alloc("java/util/stream/Stream", 0, Data::Stream(Vec::new()));
            collect_stream(&mut ctx, stream, collector);
            if collector == bad {
                let error = ctx.rt.take_exception().unwrap();
                assert_eq!(
                    ctx.rt.class_of(error),
                    Some("java/lang/NullPointerException")
                );
            } else {
                assert!(ctx.rt.pending_exception.is_none());
            }
            collect_stream(&mut ctx, stream, good);
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(
                ctx.rt.class_of(error),
                Some("java/lang/IllegalStateException")
            );
        }
    }
    fn fail(ctx: &mut ExecCtx<'_>, _: &[V]) -> V {
        ctx.rt
            .throw_exception("test/SequenceFailure", "toString callback")
    }
    fn sequence_length(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        V::I(
            ctx.rt
                .str_of(ctx.rt.get_field(args[0], "content"))
                .unwrap()
                .encode_utf16()
                .count() as i32,
        )
    }
    fn sequence_char(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        V::I(
            ctx.rt
                .str_of(ctx.rt.get_field(args[0], "content"))
                .unwrap()
                .encode_utf16()
                .nth(args[1].i() as usize)
                .unwrap() as i32,
        )
    }
    fn sequence_to_string(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let calls = ctx.rt.get_field(args[0], "toStringCalls").i();
        ctx.rt.put_field(args[0], "toStringCalls", V::I(calls + 1));
        ctx.rt.intern_str("mapped")
    }
    #[test]
    fn plain_joining_traverses_charsequence_while_delimited_joining_uses_to_string() {
        let mut rt = Rt::new();
        let mut linked = LinkedSpace::new();
        let mut host = NullHost;
        linked.register_native_module(NativeModuleRegistry {
            classes: &[NativeClassDef {
                name: "test/Sequence",
                superclass: Some("java/lang/Object"),
                interfaces: &["java/lang/CharSequence"],
                is_interface: false,
                methods: &[
                    ("length.()I", sequence_length, false, false),
                    ("charAt.(I)C", sequence_char, false, false),
                    (
                        "toString.()Ljava/lang/String;",
                        sequence_to_string,
                        false,
                        false,
                    ),
                ],
            }],
            lambdas: &[],
        });
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let sequence = ctx.rt.new_instance("test/Sequence", 0);
        let content = ctx.rt.new_str("𐐀xy");
        ctx.rt.put_field(sequence, "content", content);
        let plain = joining(&mut ctx, &[]);
        let result = collect(&mut ctx, vec![sequence], plain);
        assert_eq!(ctx.rt.str_of(result), Some("𐐀xy"));
        assert_eq!(ctx.rt.get_field(sequence, "toStringCalls").i(), 0);
        let delimiter = ctx.rt.new_str("|");
        let delimited = joining(&mut ctx, &[delimiter]);
        let result = collect(&mut ctx, vec![sequence], delimited);
        assert_eq!(ctx.rt.str_of(result), Some("mapped"));
        assert_eq!(ctx.rt.get_field(sequence, "toStringCalls"), V::I(1));
        let list = invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                "java/util/stream/Collectors",
                "toList",
                "()Ljava/util/stream/Collector;",
            ),
            &[],
        );
        let result = collect(&mut ctx, vec![content], list);
        assert!(
            matches!(&ctx.rt.get_obj(result).unwrap().data,Data::List(values)if values==&vec![content])
        );
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn real_charsequence_callbacks_propagate_and_wrong_stream_elements_throw() {
        let mut rt = Rt::new();
        let mut linked = LinkedSpace::new();
        let mut host = NullHost;
        linked.register_native_module(NativeModuleRegistry {
            classes: &[NativeClassDef {
                name: "test/FailingSequence",
                superclass: Some("java/lang/Object"),
                interfaces: &["java/lang/CharSequence"],
                is_interface: false,
                methods: &[("toString.()Ljava/lang/String;", fail, false, false)],
            }],
            lambdas: &[],
        });
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let delim = ctx.rt.new_str(",");
        let collector = joining(&mut ctx, &[delim]);
        let failing = ctx.rt.new_instance("test/FailingSequence", 0);
        collect(&mut ctx, vec![failing], collector);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(error), Some("test/SequenceFailure"));
        let wrong = ctx.rt.new_instance("test/NotCharSequence", 0);
        collect(&mut ctx, vec![wrong], collector);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(error), Some("java/lang/ClassCastException"));
    }
}
