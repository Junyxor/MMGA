//! `mmga-classfile`: JVMS Chapter 4 & 6 `.class` file parser, bytecode instruction decoder,
//! NeoForge `neoforge.mods.toml` & Jar-in-Jar metadata reader, topological dependency resolver,
//! and SpongePowered Mixin planner & weaver.

pub mod class;
pub mod insn;
pub mod jar;
pub mod metadata;
pub mod mixin;
pub mod mutf8;

pub use class::{
    access, Annotation, BootstrapArg, BootstrapMethod, ClassFile, CodeAttr, Const, ConstantValue,
    ElementValue, ExceptionHandler, FieldInfo, InvokeDynamicSite, LocalVarEntry, MemberRef,
    MethodInfo,
};
pub use insn::{
    decode_instructions, op, parse_field_descriptor, parse_method_descriptor, DescriptorType, Insn,
    Operand,
};
pub use jar::{
    scan_subscribe_methods, DiscoveredModEntrypoint, DiscoveredSubscribeMethod,
    DiscoveredSubscriber, ModJar, SubscriberBusTarget,
};
pub use metadata::{
    parse_manifest_mf, resolve_mod_load_order, DependencyOrdering, DependencyType, JarJarMetadata,
    MixinConfig, MixinDeclaration, ModDependency, ModEntry, ModLoaderKind, ModSide, ModsToml,
};
pub use mixin::{
    apply_mixin_structural_weave, parse_mixin_member_selector, plan_mixins, split_method_selector,
    InjectionPoint, InjectorKind, MixinWeaveReport, PlannedHook, SyntheticBridge, TargetWeavePlan,
};
pub use mutf8::{decode_mutf8, encode_mutf8};
