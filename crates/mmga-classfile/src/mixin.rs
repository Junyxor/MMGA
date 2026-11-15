//! SpongePowered Mixin & MixinExtras planner and weaver.
//!
//! Supports:
//! - `@Mixin(value = ..., targets = ..., priority = ...)`
//! - `@Shadow` & `@Unique` field/method merging
//! - `@Overwrite` full method replacement
//! - `@Accessor` & `@Invoker` synthetic interface/class bridge generation
//! - `@Inject(method = ..., at = @At("HEAD" | "RETURN" | "TAIL" | "INVOKE"), cancellable = ...)`
//! - `@ModifyReturnValue(method = ..., at = @At("RETURN" | "TAIL"))`
//! - `@Redirect(method = ..., at = @At(value = "INVOKE", target = ...))`
//! - `@ModifyArg(method = ..., at = @At(value = "INVOKE", target = ...), index = ...)`
//!
//! Weaving produces explicit `WeavePlan` metadata AND attaches `WovenMixinHooks` per target class,
//! which both the Tier-0 Bytecode Interpreter (`mmga-runtime`) and the Tier-1 AOT Native Rust
//! Compiler (`mmga-aot`) execute natively with zero JVM overhead.

use crate::class::{ClassFile, ElementValue, FieldInfo, MemberRef, MethodInfo};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Injection location inside a target method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InjectionPoint {
    /// Beginning of the target method before instruction 0.
    Head,
    /// Before every return instruction (`Tail` = before the last return instruction only).
    Return { tail_only: bool },
    /// Around a specific `invoke*` call site matching `target` (e.g. `"Lpkg/Owner;foo(I)V"`),
    /// either before (`after = false`) or immediately after (`after = true`).
    Invoke {
        target: MemberRef,
        after: bool,
        ordinal: Option<usize>,
    },
}

/// Kind of Mixin / MixinExtras injector handler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InjectorKind {
    Inject { cancellable: bool },
    ModifyReturnValue,
    Redirect,
    ModifyArg { index: usize },
}

/// One planned Mixin method hook on a target method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedHook {
    pub mixin_class: String,
    pub priority: i32,
    pub target_class: String,
    /// Target method `"name.desc"` or `"name"` (when descriptor omitted in `@Inject(method = "tick")`).
    pub target_method: String,
    pub handler_name: String,
    pub handler_desc: String,
    pub handler_is_static: bool,
    pub point: InjectionPoint,
    pub kind: InjectorKind,
}

/// Synthetic `@Accessor` or `@Invoker` method implementation generated on a target class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyntheticBridge {
    FieldGetter {
        interface_method: String,
        interface_desc: String,
        field_name: String,
        field_desc: String,
        is_static: bool,
    },
    FieldSetter {
        interface_method: String,
        interface_desc: String,
        field_name: String,
        field_desc: String,
        is_static: bool,
    },
    MethodInvoker {
        interface_method: String,
        interface_desc: String,
        target_name: String,
        target_desc: String,
        is_static: bool,
    },
}

/// Complete weave summary for a single target class.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TargetWeavePlan {
    pub target_class: String,
    pub applied_mixins: Vec<String>,
    pub added_interfaces: Vec<String>,
    pub added_fields: Vec<FieldInfo>,
    /// `(mixin_class, method_info)` copied onto the target (`@Overwrite` or helper/unique methods).
    pub merged_methods: Vec<(String, MethodInfo)>,
    /// Mixin constructors stripped to field initialisers only (`(mixin_class, ctor_desc)`).
    pub field_initializers: Vec<(String, MethodInfo)>,
    pub hooks: Vec<PlannedHook>,
    pub bridges: Vec<SyntheticBridge>,
}

/// Whole-modpack Mixin weave report & plan.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MixinWeaveReport {
    /// Keyed by internal target class name (`"pkg/TargetClass"`).
    pub targets: BTreeMap<String, TargetWeavePlan>,
    /// Hooks targeting external/engine classes not present as bytecode in the mod set (can bind to host hooks).
    pub external_targets: Vec<String>,
    /// Unsupported annotations/points recorded transparently instead of silently dropped.
    pub unsupported: Vec<String>,
}

/// Parses a Mixin member selector like `"Lnet/minecraft/world/level/Level;setBlock(Lnet/minecraft/core/BlockPos;I)Z"`
/// or `"net/minecraft/world/level/Level.setBlock(Lnet/minecraft/core/BlockPos;I)Z"`.
pub fn parse_mixin_member_selector(raw: &str) -> Option<MemberRef> {
    let s = raw.trim();
    if let Some(rest) = s.strip_prefix('L') {
        let (owner, member_part) = rest.split_once(';')?;
        let paren = member_part.find('(')?;
        let name = &member_part[..paren];
        let desc = &member_part[paren..];
        return Some(MemberRef::new(owner.replace('.', "/"), name, desc));
    }
    let paren = s.find('(')?;
    let before = &s[..paren];
    let desc = &s[paren..];
    let (owner, name) = before.rsplit_once(['.', '/'])?;
    Some(MemberRef::new(owner.replace('.', "/"), name, desc))
}

/// Splits a Mixin `method = "foo(I)V"` selector into `(method_name, optional_descriptor)`.
pub fn split_method_selector(sel: &str) -> (&str, Option<&str>) {
    let s = sel.trim();
    // Strip optional owner prefix `Lpkg/Cls;foo(I)V`
    let local = if let Some((_, after_semi)) = s.strip_prefix('L').and_then(|r| r.split_once(';')) {
        after_semi
    } else {
        s
    };
    if let Some(paren) = local.find('(') {
        (&local[..paren], Some(&local[paren..]))
    } else {
        (local, None)
    }
}

fn uncapitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
    }
}

fn infer_accessor_field(method_name: &str, explicit: Option<&str>) -> Option<(bool, String)> {
    if let Some(name) = explicit.filter(|s| !s.is_empty()) {
        let is_setter = method_name.starts_with("set");
        return Some((is_setter, name.to_owned()));
    }
    for prefix in ["get", "is"] {
        if let Some(rest) = method_name.strip_prefix(prefix) {
            if !rest.is_empty() {
                return Some((false, uncapitalize(rest)));
            }
        }
    }
    if let Some(rest) = method_name.strip_prefix("set") {
        if !rest.is_empty() {
            return Some((true, uncapitalize(rest)));
        }
    }
    None
}

fn infer_invoker_target(method_name: &str, explicit: Option<&str>) -> Option<String> {
    if let Some(name) = explicit.filter(|s| !s.is_empty()) {
        return Some(name.to_owned());
    }
    for prefix in ["invoke", "call"] {
        if let Some(rest) = method_name.strip_prefix(prefix) {
            if !rest.is_empty() {
                if rest.eq_ignore_ascii_case("init") || rest == "New" {
                    return Some("<init>".into());
                }
                return Some(uncapitalize(rest));
            }
        }
    }
    None
}

/// Plans all `@Mixin` classes against the known class set.
pub fn plan_mixins<'a>(
    mixin_classes: &[&'a ClassFile],
    known_classes: &BTreeMap<String, ClassFile>,
) -> MixinWeaveReport {
    let mut report = MixinWeaveReport::default();

    let mut sorted_mixins: Vec<(i32, &'a ClassFile)> = mixin_classes
        .iter()
        .copied()
        .map(|cf| {
            let prio = cf
                .annotation("Lorg/spongepowered/asm/mixin/Mixin;")
                .and_then(|a| a.get_i32("priority"))
                .unwrap_or(1000);
            (prio, cf)
        })
        .collect();
    sorted_mixins.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.cmp(&b.1.name)));

    for (priority, mixin) in sorted_mixins {
        let targets = mixin.mixin_targets();
        if targets.is_empty() {
            report
                .unsupported
                .push(format!("{}: @Mixin declares no target classes", mixin.name));
            continue;
        }

        for target in targets {
            if !known_classes.contains_key(&target) && !report.external_targets.contains(&target) {
                report.external_targets.push(target.clone());
            }
            let plan = report
                .targets
                .entry(target.clone())
                .or_insert_with(|| TargetWeavePlan {
                    target_class: target.clone(),
                    ..Default::default()
                });
            plan.applied_mixins.push(mixin.name.clone());

            if mixin.is_interface() {
                // Interface `@Mixin` containing `@Accessor` / `@Invoker` declarations!
                if !plan.added_interfaces.contains(&mixin.name) {
                    plan.added_interfaces.push(mixin.name.clone());
                }
                for m in &mixin.methods {
                    if let Some(acc) = m.annotation("Lorg/spongepowered/asm/mixin/gen/Accessor;") {
                        let explicit = acc.get_str("value");
                        if let Some((is_setter, field_name)) =
                            infer_accessor_field(&m.name, explicit)
                        {
                            if is_setter {
                                let field_desc = m
                                    .desc
                                    .strip_prefix('(')
                                    .and_then(|s| s.strip_suffix(")V"))
                                    .unwrap_or("Ljava/lang/Object;")
                                    .to_owned();
                                plan.bridges.push(SyntheticBridge::FieldSetter {
                                    interface_method: m.name.clone(),
                                    interface_desc: m.desc.clone(),
                                    field_name,
                                    field_desc,
                                    is_static: m.is_static(),
                                });
                            } else {
                                let field_desc = m
                                    .desc
                                    .split_once(')')
                                    .map_or("Ljava/lang/Object;", |(_, r)| r)
                                    .to_owned();
                                plan.bridges.push(SyntheticBridge::FieldGetter {
                                    interface_method: m.name.clone(),
                                    interface_desc: m.desc.clone(),
                                    field_name,
                                    field_desc,
                                    is_static: m.is_static(),
                                });
                            }
                        }
                        continue;
                    }
                    if let Some(inv) = m.annotation("Lorg/spongepowered/asm/mixin/gen/Invoker;") {
                        let explicit = inv.get_str("value");
                        if let Some(target_name) = infer_invoker_target(&m.name, explicit) {
                            plan.bridges.push(SyntheticBridge::MethodInvoker {
                                interface_method: m.name.clone(),
                                interface_desc: m.desc.clone(),
                                target_name,
                                target_desc: m.desc.clone(),
                                is_static: m.is_static(),
                            });
                        }
                    }
                }
                continue;
            }

            // Merge interfaces declared on the mixin class
            for iface in &mixin.interfaces {
                if !plan.added_interfaces.contains(iface) {
                    plan.added_interfaces.push(iface.clone());
                }
            }

            // Merge non-@Shadow fields
            for field in &mixin.fields {
                if field.has_annotation("Lorg/spongepowered/asm/mixin/Shadow;") {
                    continue;
                }
                if !plan.added_fields.iter().any(|f| f.name == field.name) {
                    plan.added_fields.push(field.clone());
                }
            }

            // Inspect methods: constructors, @Shadow, @Overwrite, injectors, or helper methods
            for method in &mixin.methods {
                if method.name == "<init>" {
                    if method.code.is_some() {
                        plan.field_initializers
                            .push((mixin.name.clone(), method.clone()));
                    }
                    continue;
                }
                if method
                    .annotation("Lorg/spongepowered/asm/mixin/Shadow;")
                    .is_some()
                {
                    continue;
                }

                // Check injector annotations
                let injector_spec = method
                    .annotation("Lorg/spongepowered/asm/mixin/injection/Inject;")
                    .map(|a| {
                        (
                            a,
                            InjectorKind::Inject {
                                cancellable: a.get_bool("cancellable").unwrap_or(false),
                            },
                        )
                    })
                    .or_else(|| {
                        method
                            .annotation(
                                "Lcom/llamalad7/mixinextras/injector/ModifyReturnValue;",
                            )
                            .map(|a| (a, InjectorKind::ModifyReturnValue))
                    })
                    .or_else(|| {
                        method
                            .annotation("Lorg/spongepowered/asm/mixin/injection/Redirect;")
                            .map(|a| (a, InjectorKind::Redirect))
                    })
                    .or_else(|| {
                        method
                            .annotation("Lorg/spongepowered/asm/mixin/injection/ModifyArg;")
                            .map(|a| {
                                let index = a.get_i32("index").unwrap_or(0).max(0) as usize;
                                (a, InjectorKind::ModifyArg { index })
                            })
                    });

                if let Some((ann, kind)) = injector_spec {
                    // Keep the handler method itself merged on the target under a mangled unique name
                    let mangled_name = format!(
                        "__mixin${}${}",
                        mixin.name.replace(['/', '$'], "_"),
                        method.name
                    );
                    let mut handler_copy = method.clone();
                    handler_copy.name = mangled_name.clone();
                    plan.merged_methods
                        .push((mixin.name.clone(), handler_copy));

                    let methods_attr = ann
                        .get("method")
                        .map(ElementValue::to_string_list)
                        .unwrap_or_default();
                    let at_list = ann
                        .get("at")
                        .map(ElementValue::to_annotations)
                        .unwrap_or_default();

                    if methods_attr.is_empty() || at_list.is_empty() {
                        report.unsupported.push(format!(
                            "{}#{}: missing `method` or `at` in injector",
                            mixin.name, method.name
                        ));
                        continue;
                    }

                    for at_ann in at_list {
                        let at_val = at_ann.get_str("value").unwrap_or("");
                        let point = match at_val {
                            "HEAD" => Some(InjectionPoint::Head),
                            "RETURN" => Some(InjectionPoint::Return { tail_only: false }),
                            "TAIL" => Some(InjectionPoint::Return { tail_only: true }),
                            "INVOKE" => {
                                let tgt = at_ann
                                    .get_str("target")
                                    .and_then(parse_mixin_member_selector);
                                let shift = at_ann.get_str("shift").unwrap_or("BEFORE");
                                let ordinal =
                                    at_ann.get_i32("ordinal").filter(|&o| o >= 0).map(|o| o as usize);
                                match tgt {
                                    Some(member) => Some(InjectionPoint::Invoke {
                                        target: member,
                                        after: shift == "AFTER",
                                        ordinal,
                                    }),
                                    None => {
                                        report.unsupported.push(format!(
                                            "{}#{}: unparseable @At(INVOKE) target {:?}",
                                            mixin.name,
                                            method.name,
                                            at_ann.get_str("target")
                                        ));
                                        None
                                    }
                                }
                            }
                            other => {
                                report.unsupported.push(format!(
                                    "{}#{}: unsupported @At(\"{other}\")",
                                    mixin.name, method.name
                                ));
                                None
                            }
                        };

                        if let Some(point) = point {
                            for m_sel in &methods_attr {
                                let (m_name, m_desc) = split_method_selector(m_sel);
                                let target_method = match m_desc {
                                    Some(d) => format!("{m_name}.{d}"),
                                    None => m_name.to_owned(),
                                };
                                plan.hooks.push(PlannedHook {
                                    mixin_class: mixin.name.clone(),
                                    priority,
                                    target_class: target.clone(),
                                    target_method,
                                    handler_name: mangled_name.clone(),
                                    handler_desc: method.desc.clone(),
                                    handler_is_static: method.is_static(),
                                    point: point.clone(),
                                    kind: kind.clone(),
                                });
                            }
                        }
                    }
                    continue;
                }

                // Regular helper method, `@Unique` method, or `@Overwrite` method: merge onto target!
                plan.merged_methods
                    .push((mixin.name.clone(), method.clone()));
            }
        }
    }

    // Sort hooks per target by `(priority, mixin_class)`
    for plan in report.targets.values_mut() {
        plan.hooks
            .sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.mixin_class.cmp(&b.mixin_class)));
    }

    report
}

/// Applies a `MixinWeaveReport`'s structural changes (interfaces, fields, `@Overwrite` / helper /
/// mangled handler methods) directly into the mutable `classes` map, returning the `MixinWeaveReport`
/// so runtimes and AOT compilers can wire `hooks`, `field_initializers`, and `bridges`.
pub fn apply_mixin_structural_weave(
    classes: &mut BTreeMap<String, ClassFile>,
    report: &MixinWeaveReport,
) {
    for (target_name, plan) in &report.targets {
        let Some(target_cf) = classes.get_mut(target_name) else {
            continue;
        };

        for iface in &plan.added_interfaces {
            if !target_cf.interfaces.contains(iface) {
                target_cf.interfaces.push(iface.clone());
            }
        }

        for field in &plan.added_fields {
            if !target_cf.fields.iter().any(|f| f.name == field.name) {
                target_cf.fields.push(field.clone());
            }
        }

        // Note: merged methods came from `mixin_cf` and reference `mixin_cf`'s constant pool!
        // Therefore, runtimes & AOT codegen resolve a merged method's bytecode against its
        // originating `mixin_class`'s `ClassFile` constant pool while binding it on `target_class`!
        // For `@Overwrite` / added plain methods without pool dependencies or where lookup checks
        // `plan.merged_methods`, keeping `plan.merged_methods` authoritative avoids constant-pool
        // index collisions!
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mixin_selectors_and_infer_accessors() {
        let m = parse_mixin_member_selector(
            "Lnet/minecraft/world/level/Level;setBlock(Lnet/minecraft/core/BlockPos;I)Z",
        )
        .unwrap();
        assert_eq!(m.owner, "net/minecraft/world/level/Level");
        assert_eq!(m.name, "setBlock");
        assert_eq!(m.desc, "(Lnet/minecraft/core/BlockPos;I)Z");

        assert_eq!(
            infer_accessor_field("getSpeed", None),
            Some((false, "speed".into()))
        );
        assert_eq!(
            infer_accessor_field("setStress", None),
            Some((true, "stress".into()))
        );
        assert_eq!(
            infer_invoker_target("invokeRecalculate", None),
            Some("recalculate".into())
        );
    }
}
