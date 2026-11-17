//! Guava 32.1.2 ImmutableMap builders used by Sophisticated Core's SortBy.
//! Null entries fail at put; duplicate keys fail at build except buildKeepingLast.
//! Builder data is field-backed so java.util.Map dispatch cannot consume its put.

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, Data, ExecCtx, HostApi, LinkedSpace, Rt, V};

const MAP: &str = "com/google/common/collect/ImmutableMap";
const BUILDER: &str = "com/google/common/collect/ImmutableMap$Builder";

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
    if owner == MAP && is_static {
        return match (name, desc) {
            ("builder", "()Lcom/google/common/collect/ImmutableMap$Builder;") => {
                Some(new_builder(rt))
            }
            ("builderWithExpectedSize", "(I)Lcom/google/common/collect/ImmutableMap$Builder;") => {
                Some(if args[0].i() < 0 {
                    rt.throw_exception(
                        "java/lang/IllegalArgumentException",
                        "expectedSize cannot be negative",
                    )
                } else {
                    new_builder(rt)
                })
            }
            _ => None,
        };
    }
    if owner != BUILDER || is_static {
        return None;
    }
    let receiver = *args.first()?;
    if name == "<init>" && desc == "()V" {
        initialize(rt, receiver);
        return Some(V::Void);
    }
    let store = rt.get_field(receiver, "$entries");
    if store.is_null() {
        return None;
    }
    let mut ctx = ExecCtx {
        rt,
        linked,
        host,
        depth: 0,
    };
    match(name,desc){
        ("put","(Ljava/lang/Object;Ljava/lang/Object;)Lcom/google/common/collect/ImmutableMap$Builder;")=>Some(put(ctx.rt,receiver,args[1],args[2])),
        ("put","(Ljava/util/Map$Entry;)Lcom/google/common/collect/ImmutableMap$Builder;")=>{
            let key=invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new("java/util/Map$Entry","getKey","()Ljava/lang/Object;"),&[args[1]]);if ctx.rt.pending_exception.is_some(){return Some(V::NULL);}
            let value=invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new("java/util/Map$Entry","getValue","()Ljava/lang/Object;"),&[args[1]]);if ctx.rt.pending_exception.is_some(){return Some(V::NULL);}Some(put(ctx.rt,receiver,key,value))
        }
        ("putAll","(Ljava/util/Map;)Lcom/google/common/collect/ImmutableMap$Builder;")=>{
            if args[1].is_null(){return Some(ctx.rt.throw_exception("java/lang/NullPointerException","putAll map is null"));}
            let Some(Data::Map(entries))=ctx.rt.get_obj(args[1]).map(|o|&o.data)else{return None;};let entries=entries.clone();
            for(key,value)in entries{put(ctx.rt,receiver,key,value);if ctx.rt.pending_exception.is_some(){return Some(V::NULL);}}Some(receiver)
        }
        ("build"|"buildOrThrow"|"buildKeepingLast","()Lcom/google/common/collect/ImmutableMap;")=>{
            let Some(Data::Map(entries))=ctx.rt.get_obj(store).map(|o|&o.data)else{return None;};let entries=entries.clone();let mut unique:Vec<(V,V)>=Vec::new();
            for(key,value)in entries{
                let mut duplicate=None;
                for(index,(existing,_))in unique.iter().enumerate(){let equal=invoke_member(&mut ctx,op::INVOKEVIRTUAL,&MemberRef::new("java/lang/Object","equals","(Ljava/lang/Object;)Z"),&[key,*existing]);if ctx.rt.pending_exception.is_some(){return Some(V::NULL);}if equal.i()!=0{duplicate=Some(index);break;}}
                if let Some(index)=duplicate{
                    if name!="buildKeepingLast"{return Some(ctx.rt.throw_exception("java/lang/IllegalArgumentException",&format!("Multiple entries with same key: {}",ctx.rt.value_to_string(key))));}
                    unique[index].1=value;
                }else{unique.push((key,value));}
            }
            Some(ctx.rt.alloc(MAP,0,Data::Map(unique)))
        }
        _=>None,
    }
}

fn initialize(rt: &mut Rt, receiver: V) {
    let entries = rt.alloc("java/util/ArrayMap", 0, Data::Map(Vec::new()));
    rt.put_field(receiver, "$entries", entries);
}
fn new_builder(rt: &mut Rt) -> V {
    let builder = rt.new_instance(BUILDER, 0);
    initialize(rt, builder);
    builder
}
fn put(rt: &mut Rt, receiver: V, key: V, value: V) -> V {
    if key.is_null() || value.is_null() {
        return rt.throw_exception(
            "java/lang/NullPointerException",
            "null key or value in ImmutableMap.Builder",
        );
    }
    let store = rt.get_field(receiver, "$entries");
    if let Some(Data::Map(entries)) = rt.get_obj_mut(store).map(|o| &mut o.data) {
        entries.push((key, value));
    }
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;
    struct BuilderHost;
    impl HostApi for BuilderHost {
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
            call(rt, linked, self, member, is_static, args)
        }
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
    fn direct_builder_constructor_keeps_entries_and_build_snapshot_independent() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = BuilderHost;
        let builder = rt.new_instance(BUILDER, 0);
        let key = rt.intern_str("name");
        let value = rt.intern_str("first");
        let other = rt.intern_str("second");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(BUILDER, "<init>", "()V"),
            &[builder],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(invoke(&mut ctx,BUILDER,"put","(Ljava/lang/Object;Ljava/lang/Object;)Lcom/google/common/collect/ImmutableMap$Builder;",&[builder,key,value]),builder);
        let first = invoke(
            &mut ctx,
            BUILDER,
            "build",
            "()Lcom/google/common/collect/ImmutableMap;",
            &[builder],
        );
        invoke(&mut ctx,BUILDER,"put","(Ljava/lang/Object;Ljava/lang/Object;)Lcom/google/common/collect/ImmutableMap$Builder;",&[builder,other,value]);
        assert_eq!(
            invoke(&mut ctx, "java/util/Map", "size", "()I", &[first]),
            V::I(1)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Map",
                "get",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[first, key]
            ),
            value
        );
        assert!(ctx.rt.pending_exception.is_none());
        invoke(&mut ctx, "java/util/Map", "clear", "()V", &[first]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
    #[test]
    fn null_put_is_atomic_and_duplicate_build_does_not_rewrite_builder() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = BuilderHost;
        let builder = rt.new_instance(BUILDER, 0);
        let key = rt.intern_str("name");
        let first = rt.intern_str("first");
        let last = rt.intern_str("last");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(BUILDER, "<init>", "()V"),
            &[builder],
        );
        let put="(Ljava/lang/Object;Ljava/lang/Object;)Lcom/google/common/collect/ImmutableMap$Builder;";
        invoke(&mut ctx, BUILDER, "put", put, &[builder, key, V::NULL]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
        invoke(&mut ctx, BUILDER, "put", put, &[builder, key, first]);
        invoke(&mut ctx, BUILDER, "put", put, &[builder, key, last]);
        let kept = invoke(
            &mut ctx,
            BUILDER,
            "buildKeepingLast",
            "()Lcom/google/common/collect/ImmutableMap;",
            &[builder],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Map",
                "get",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[kept, key]
            ),
            last
        );
        invoke(
            &mut ctx,
            BUILDER,
            "buildOrThrow",
            "()Lcom/google/common/collect/ImmutableMap;",
            &[builder],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("IllegalArgumentException"));
    }
}
