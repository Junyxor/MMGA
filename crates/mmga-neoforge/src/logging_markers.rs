//! Log4j 2.22.1 MarkerManager value semantics (Minecraft 1.21.1 dependency).
//!
//! Official artifact SHA-1 bea6fede6328fabafd7e68363161a7ea6605abd1;
//! Apache source tag rel/2.22.1 commit 8469975a4f2b1f8f1bd4f25ca6d1989a52aefc1b.
//! Markers retain identity and parent graphs. No logger enablement/filtering is
//! inferred from the existence of a marker.

use mmga_classfile::MemberRef;
use mmga_runtime::{Data, Rt, V};
const MANAGER: &str = "org/apache/logging/log4j/MarkerManager";
const MARKER: &str = "org/apache/logging/log4j/Marker";
const IMPLEMENTATION: &str = "org/apache/logging/log4j/MarkerManager$Log4jMarker";
const ARRAY: &str = "[Lorg/apache/logging/log4j/Marker;";
const CACHE: &str = "mmga/log4j/markers";

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if is_static {
        if member.owner != MANAGER {
            return None;
        }
        return match(name,desc){
            ("getMarker","(Ljava/lang/String;)Lorg/apache/logging/log4j/Marker;")=>Some(get_marker(rt,*args.first()?)),
            ("getMarker","(Ljava/lang/String;Ljava/lang/String;)Lorg/apache/logging/log4j/Marker;")=>{
                let parent_name=*args.get(1)?;
                if parent_name.is_null(){return Some(rt.throw_exception("java/lang/NullPointerException","MarkerManager parent name"));}
                let parent_text=rt.str_of(parent_name)?.to_owned();
                let Some(parent)=rt.get_static(CACHE,&parent_text) else{return Some(rt.throw_exception("java/lang/IllegalArgumentException",&format!("Parent Marker {parent_text} has not been defined")));};
                let value=get_marker(rt,args[0]);if rt.pending_exception.is_some(){return Some(V::NULL);}Some(add_parents(rt,value,vec![parent]))
            }
            ("getMarker","(Ljava/lang/String;Lorg/apache/logging/log4j/Marker;)Lorg/apache/logging/log4j/Marker;")=>{
                let value=get_marker(rt,*args.first()?);if rt.pending_exception.is_some(){return Some(V::NULL);}Some(add_parents(rt,value,vec![*args.get(1)?]))
            }
            ("exists","(Ljava/lang/String;)Z")=>{
                let value=*args.first()?;if value.is_null(){return Some(rt.throw_exception("java/lang/NullPointerException","MarkerManager name"));}
                Some(V::bool(rt.get_static(CACHE,rt.str_of(value)?).is_some()))
            }
            ("clear","()V")=>{rt.statics.retain(|key,_|!key.starts_with("mmga/log4j/markers."));Some(V::Void)}
            _=>None,
        };
    }
    let recv = *args.first()?;
    if member.owner == IMPLEMENTATION && name == "<init>" && desc == "(Ljava/lang/String;)V" {
        let value = *args.get(1)?;
        if value.is_null() {
            return Some(rt.throw_exception(
                "java/lang/IllegalArgumentException",
                "Marker name cannot be null.",
            ));
        }
        rt.put_field(recv, "name", value);
        rt.put_field(recv, "parents", V::NULL);
        return Some(V::Void);
    }
    if rt.class_of(recv) != Some(IMPLEMENTATION) {
        return None;
    }
    match (name, desc) {
        ("getName", "()Ljava/lang/String;") => Some(rt.get_field(recv, "name")),
        ("getParents", "()[Lorg/apache/logging/log4j/Marker;") => {
            let parents = rt.get_field(recv, "parents");
            Some(if parents.is_null() {
                V::NULL
            } else {
                let values = array(rt, parents)?;
                rt.alloc(ARRAY, 0, Data::Array(values))
            })
        }
        ("hasParents", "()Z") => Some(V::bool(!rt.get_field(recv, "parents").is_null())),
        ("setParents", "([Lorg/apache/logging/log4j/Marker;)Lorg/apache/logging/log4j/Marker;") => {
            let value = *args.get(1)?;
            let values = if value.is_null() {
                Vec::new()
            } else {
                array(rt, value)?
            };
            let parents = if values.is_empty() {
                V::NULL
            } else {
                rt.alloc(ARRAY, 0, Data::Array(values))
            };
            rt.put_field(recv, "parents", parents);
            Some(recv)
        }
        ("addParents", "([Lorg/apache/logging/log4j/Marker;)Lorg/apache/logging/log4j/Marker;") => {
            let value = *args.get(1)?;
            if value.is_null() {
                return Some(rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    "A parent marker must be specified",
                ));
            }
            Some(add_parents(rt, recv, array(rt, value)?))
        }
        ("remove", "(Lorg/apache/logging/log4j/Marker;)Z") => {
            let parent = *args.get(1)?;
            if parent.is_null() {
                return Some(rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    "A parent marker must be specified",
                ));
            }
            let existing = rt.get_field(recv, "parents");
            if existing.is_null() {
                return Some(V::FALSE);
            }
            let values = array(rt, existing)?;
            if values.is_empty() {
                return Some(rt.throw_exception("java/lang/NegativeArraySizeException", "-1"));
            }
            let mut survivors = Vec::new();
            for value in &values {
                let equal = match same_name(rt, *value, parent) {
                    Ok(v) => v,
                    Err(e) => return Some(e),
                };
                if !equal {
                    survivors.push(*value);
                }
            }
            if survivors.len() == values.len() {
                return Some(V::FALSE);
            }
            let new_parents = if values.len() == 1 {
                V::NULL
            } else {
                // Log4j allocates length-1 even when duplicate names are removed.
                survivors.resize(values.len() - 1, V::NULL);
                rt.alloc(ARRAY, 0, Data::Array(survivors))
            };
            rt.put_field(recv, "parents", new_parents);
            Some(V::TRUE)
        }
        ("isInstanceOf", "(Lorg/apache/logging/log4j/Marker;)Z") => {
            let parent = *args.get(1)?;
            if parent.is_null() {
                return Some(rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    "A marker parameter is required",
                ));
            }
            Some(match ancestor(rt, recv, parent, &mut Vec::new()) {
                Ok(value) => V::bool(value),
                Err(error) => error,
            })
        }
        ("isInstanceOf", "(Ljava/lang/String;)Z") => {
            let name = *args.get(1)?;
            if name.is_null() {
                return Some(rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    "A marker name is required",
                ));
            }
            let text = rt.str_of(name)?.to_owned();
            if rt.str_of(rt.get_field(recv, "name")) == Some(&text) {
                return Some(V::TRUE);
            }
            let Some(parent) = rt.get_static(CACHE, &text) else {
                return Some(V::FALSE);
            };
            Some(match ancestor(rt, recv, parent, &mut Vec::new()) {
                Ok(value) => V::bool(value),
                Err(error) => error,
            })
        }
        ("equals", "(Ljava/lang/Object;)Z") => Some(match same_name(rt, recv, *args.get(1)?) {
            Ok(v) => V::bool(v),
            Err(e) => e,
        }),
        ("hashCode", "()I") => {
            let name = rt.str_of(rt.get_field(recv, "name"))?;
            Some(V::I(name.encode_utf16().fold(0i32, |sum, c| {
                sum.wrapping_mul(31).wrapping_add(c as i32)
            })))
        }
        ("toString", "()Ljava/lang/String;") => Some(match render(rt, recv, &mut Vec::new()) {
            Ok(text) => rt.new_str(text),
            Err(error) => error,
        }),
        ("formatTo", "(Ljava/lang/StringBuilder;)V") => {
            let buffer = *args.get(1)?;
            if buffer.is_null() {
                return Some(
                    rt.throw_exception("java/lang/NullPointerException", "Marker.formatTo(null)"),
                );
            }
            let text = match render(rt, recv, &mut Vec::new()) {
                Ok(v) => v,
                Err(e) => return Some(e),
            };
            if let Some(Data::StringBuilder(value)) = rt.get_obj_mut(buffer).map(|o| &mut o.data) {
                value.push_str(&text);
                Some(V::Void)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn get_marker(rt: &mut Rt, name: V) -> V {
    if name.is_null() {
        return rt.throw_exception("java/lang/NullPointerException", "MarkerManager name");
    }
    let Some(text) = rt.str_of(name).map(str::to_owned) else {
        return rt.throw_exception(
            "java/lang/ClassCastException",
            "Marker name must be a String",
        );
    };
    if let Some(value) = rt.get_static(CACHE, &text) {
        return value;
    }
    let value = rt.new_instance(IMPLEMENTATION, 0);
    rt.put_field(value, "name", name);
    rt.put_field(value, "parents", V::NULL);
    rt.put_static(CACHE, &text, value);
    value
}
fn array(rt: &Rt, value: V) -> Option<Vec<V>> {
    match rt.get_obj(value).map(|o| &o.data) {
        Some(Data::Array(v)) => Some(v.clone()),
        _ => None,
    }
}
fn add_parents(rt: &mut Rt, recv: V, parents: Vec<V>) -> V {
    let old = rt.get_field(recv, "parents");
    if old.is_null() {
        let array = rt.alloc(ARRAY, 0, Data::Array(parents));
        rt.put_field(recv, "parents", array);
        return recv;
    }
    let old_values = array(rt, old).expect("modeled marker parent array");
    let mut extra = Vec::new();
    for parent in parents {
        if old_values.contains(&parent) {
            continue;
        }
        if parent.is_null() {
            return rt.throw_exception("java/lang/NullPointerException", "null parent marker");
        }
        match ancestor(rt, parent, recv, &mut Vec::new()) {
            Ok(false) => extra.push(parent),
            Ok(true) => {}
            Err(error) => return error,
        }
    }
    if extra.is_empty() {
        return recv;
    }
    let mut values = old_values;
    values.extend(extra);
    let result = rt.alloc(ARRAY, 0, Data::Array(values));
    rt.put_field(recv, "parents", result);
    recv
}
fn same_name(rt: &mut Rt, left: V, right: V) -> Result<bool, V> {
    if left == right {
        return Ok(true);
    }
    if left.is_null() {
        return Err(rt.throw_exception("java/lang/NullPointerException", "null parent marker"));
    }
    if right.is_null() {
        return Ok(false);
    }
    if matches!(
        rt.class_of(right),
        Some(
            "java/lang/Object"
                | "java/lang/String"
                | "java/lang/Integer"
                | "java/lang/Long"
                | "java/lang/Boolean"
                | "java/lang/Float"
                | "java/lang/Double"
                | "java/lang/Class"
        )
    ) {
        return Ok(false);
    }
    if rt.class_of(left) != Some(IMPLEMENTATION) || rt.class_of(right) != Some(IMPLEMENTATION) {
        return Err(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "MMGA cannot inspect external Marker implementation",
        ));
    }
    Ok(rt.str_of(rt.get_field(left, "name")) == rt.str_of(rt.get_field(right, "name")))
}
fn ancestor(rt: &mut Rt, node: V, target: V, path: &mut Vec<V>) -> Result<bool, V> {
    if node == target {
        return Ok(true);
    }
    if node.is_null() {
        return Err(rt.throw_exception("java/lang/NullPointerException", "null parent marker"));
    }
    if path.contains(&node) {
        return Err(rt.throw_exception(
            "java/lang/StackOverflowError",
            "cyclic Log4j marker ancestry",
        ));
    }
    if rt.class_of(node) != Some(IMPLEMENTATION) {
        return Err(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "MMGA cannot inspect external Marker ancestry",
        ));
    }
    let parents = rt.get_field(node, "parents");
    if parents.is_null() {
        return Ok(false);
    }
    path.push(node);
    for parent in array(rt, parents).expect("modeled parent array") {
        if ancestor(rt, parent, target, path)? {
            path.pop();
            return Ok(true);
        }
    }
    path.pop();
    Ok(false)
}
fn render(rt: &mut Rt, node: V, path: &mut Vec<V>) -> Result<String, V> {
    if node.is_null() {
        return Err(rt.throw_exception("java/lang/NullPointerException", "null parent marker"));
    }
    if path.contains(&node) {
        return Err(rt.throw_exception(
            "java/lang/StackOverflowError",
            "cyclic Log4j marker formatting",
        ));
    }
    if rt.class_of(node) != Some(IMPLEMENTATION) {
        return Err(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "MMGA cannot format external Marker implementation",
        ));
    }
    let mut text = rt
        .str_of(rt.get_field(node, "name"))
        .unwrap_or("null")
        .to_owned();
    let parents = rt.get_field(node, "parents");
    if !parents.is_null() {
        path.push(node);
        let mut children = Vec::new();
        for parent in array(rt, parents).expect("modeled parent array") {
            children.push(render(rt, parent, path)?);
        }
        path.pop();
        text.push_str("[ ");
        text.push_str(&children.join(", "));
        text.push_str(" ]");
    }
    Ok(text)
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == IMPLEMENTATION
        && matches!(
            sup,
            MARKER
                | "java/io/Serializable"
                | "org/apache/logging/log4j/util/StringBuilderFormattable"
        ))
        || (sub == MARKER && sup == "java/io/Serializable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, Data, ExecCtx, HostApi, LinkedSpace};
    struct MarkerHost;
    impl HostApi for MarkerHost {
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
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            call(rt, member, is_static, args)
        }
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            is_subtype(sub, sup)
        }
    }
    fn invoke(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(MARKER, name, desc),
            args,
        )
    }
    fn marker(ctx: &mut ExecCtx<'_>, name: &str) -> V {
        let name = ctx.rt.intern_str(name);
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                MANAGER,
                "getMarker",
                "(Ljava/lang/String;)Lorg/apache/logging/log4j/Marker;",
            ),
            &[name],
        )
    }
    fn parents(rt: &Rt, value: V) -> Vec<V> {
        match &rt.get_obj(value).unwrap().data {
            Data::Array(v) => v.clone(),
            _ => panic!("array"),
        }
    }

    #[test]
    fn marker_manager_interns_names_but_clear_does_not_mutate_existing_objects() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = MarkerHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let first = marker(&mut ctx, "REGISTRATE.REGISTER");
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(first, marker(&mut ctx, "REGISTRATE.REGISTER"));
        let name = invoke(&mut ctx, "getName", "()Ljava/lang/String;", &[first]);
        assert_eq!(ctx.rt.str_of(name), Some("REGISTRATE.REGISTER"));
        assert_eq!(
            invoke(
                &mut ctx,
                "getParents",
                "()[Lorg/apache/logging/log4j/Marker;",
                &[first]
            ),
            V::NULL
        );
        invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(MANAGER, "clear", "()V"),
            &[],
        );
        let second = marker(&mut ctx, "REGISTRATE.REGISTER");
        assert_ne!(first, second);
        assert_eq!(
            invoke(
                &mut ctx,
                "equals",
                "(Ljava/lang/Object;)Z",
                &[first, second]
            ),
            V::TRUE
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "isInstanceOf",
                "(Lorg/apache/logging/log4j/Marker;)Z",
                &[first, second]
            ),
            V::FALSE
        );
    }

    #[test]
    fn marker_parents_are_copied_and_ancestry_uses_marker_identity() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = MarkerHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let root = marker(&mut ctx, "ROOT");
        let child = marker(&mut ctx, "CHILD");
        let leaf = marker(&mut ctx, "LEAF");
        let array = ctx.rt.alloc(ARRAY, 0, Data::Array(vec![root]));
        invoke(
            &mut ctx,
            "setParents",
            "([Lorg/apache/logging/log4j/Marker;)Lorg/apache/logging/log4j/Marker;",
            &[child, array],
        );
        let next = ctx.rt.alloc(ARRAY, 0, Data::Array(vec![child]));
        invoke(
            &mut ctx,
            "addParents",
            "([Lorg/apache/logging/log4j/Marker;)Lorg/apache/logging/log4j/Marker;",
            &[leaf, next],
        );
        assert!(ctx.rt.pending_exception.is_none());
        if let Data::Array(v) = &mut ctx.rt.get_obj_mut(array).unwrap().data {
            v.clear();
        }
        assert_eq!(
            invoke(
                &mut ctx,
                "isInstanceOf",
                "(Lorg/apache/logging/log4j/Marker;)Z",
                &[leaf, root]
            ),
            V::TRUE
        );
        let root_name = ctx.rt.intern_str("ROOT");
        assert_eq!(
            invoke(
                &mut ctx,
                "isInstanceOf",
                "(Ljava/lang/String;)Z",
                &[leaf, root_name]
            ),
            V::TRUE
        );
        let snapshot = invoke(
            &mut ctx,
            "getParents",
            "()[Lorg/apache/logging/log4j/Marker;",
            &[child],
        );
        assert_eq!(parents(ctx.rt, snapshot), vec![root]);
        if let Data::Array(v) = &mut ctx.rt.get_obj_mut(snapshot).unwrap().data {
            v.clear();
        }
        let text = invoke(&mut ctx, "toString", "()Ljava/lang/String;", &[leaf]);
        assert_eq!(ctx.rt.str_of(text), Some("LEAF[ CHILD[ ROOT ] ]"));
        assert_eq!(
            invoke(
                &mut ctx,
                "remove",
                "(Lorg/apache/logging/log4j/Marker;)Z",
                &[child, root]
            ),
            V::TRUE
        );
        assert_eq!(invoke(&mut ctx, "hasParents", "()Z", &[child]), V::FALSE);
        assert_eq!(
            invoke(
                &mut ctx,
                "isInstanceOf",
                "(Lorg/apache/logging/log4j/Marker;)Z",
                &[leaf, root]
            ),
            V::FALSE
        );
    }

    #[test]
    fn marker_null_and_empty_parent_cases_match_log4j_api() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = MarkerHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let value = marker(&mut ctx, "");
        assert!(ctx.rt.pending_exception.is_none());
        invoke(
            &mut ctx,
            "addParents",
            "([Lorg/apache/logging/log4j/Marker;)Lorg/apache/logging/log4j/Marker;",
            &[value, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("IllegalArgumentException"));
        let empty = ctx.rt.alloc(ARRAY, 0, Data::Array(Vec::new()));
        invoke(
            &mut ctx,
            "addParents",
            "([Lorg/apache/logging/log4j/Marker;)Lorg/apache/logging/log4j/Marker;",
            &[value, empty],
        );
        assert_eq!(invoke(&mut ctx, "hasParents", "()Z", &[value]), V::TRUE);
        invoke(
            &mut ctx,
            "setParents",
            "([Lorg/apache/logging/log4j/Marker;)Lorg/apache/logging/log4j/Marker;",
            &[value, empty],
        );
        assert_eq!(invoke(&mut ctx, "hasParents", "()Z", &[value]), V::FALSE);
        invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                MANAGER,
                "getMarker",
                "(Ljava/lang/String;)Lorg/apache/logging/log4j/Marker;",
            ),
            &[V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
    }

    #[test]
    fn add_parents_checks_existing_edges_and_exposes_real_cycle_failure() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = MarkerHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let a = marker(&mut ctx, "A");
        let b = marker(&mut ctx, "B");
        let other = marker(&mut ctx, "OTHER");
        let roots = ctx.rt.alloc(ARRAY, 0, Data::Array(vec![b]));
        invoke(
            &mut ctx,
            "addParents",
            "([Lorg/apache/logging/log4j/Marker;)Lorg/apache/logging/log4j/Marker;",
            &[a, roots],
        );
        invoke(
            &mut ctx,
            "addParents",
            "([Lorg/apache/logging/log4j/Marker;)Lorg/apache/logging/log4j/Marker;",
            &[a, roots],
        );
        let current = invoke(
            &mut ctx,
            "getParents",
            "()[Lorg/apache/logging/log4j/Marker;",
            &[a],
        );
        assert_eq!(parents(ctx.rt, current), vec![b]);
        let illegal = ctx.rt.alloc(ARRAY, 0, Data::Array(vec![a]));
        // Log4j's first add does not check ancestry; preserve the graph and report
        // StackOverflowError when a later traversal encounters its cycle.
        invoke(
            &mut ctx,
            "addParents",
            "([Lorg/apache/logging/log4j/Marker;)Lorg/apache/logging/log4j/Marker;",
            &[b, illegal],
        );
        invoke(
            &mut ctx,
            "isInstanceOf",
            "(Lorg/apache/logging/log4j/Marker;)Z",
            &[a, other],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("StackOverflowError"));
        let text = ctx.rt.intern_str("A");
        assert_eq!(
            invoke(&mut ctx, "equals", "(Ljava/lang/Object;)Z", &[a, text]),
            V::FALSE
        );
        assert_eq!(invoke(&mut ctx, "hashCode", "()I", &[a]), V::I(65));
    }
}
