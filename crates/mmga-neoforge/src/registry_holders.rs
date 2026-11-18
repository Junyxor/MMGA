//! Canonical registry reference and intrusive preregistration state.
//! Checked against 1.21.1 MappedRegistry/Holder.Reference bytecode and the
//! official NeoForge MappedRegistry patch (immediate binding, duplicate throws).
use crate::{canonical_registry_key, ContentProperties, RegisteredEntry, RegistryStore};
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, Data, ExecCtx, Rt, V};
use std::collections::BTreeMap;

const REGISTRY: &str = "net/minecraft/core/Registry";
const REFERENCE: &str = "net/minecraft/core/Holder$Reference";
const OWNER: &str = "net/minecraft/core/HolderOwner";
const INFO: &str = "net/minecraft/core/RegistrationInfo";
const BUILDER: &str = "net/neoforged/neoforge/registries/RegistryBuilder";
const CALLBACKS: &str = "mmga/internal/RegistryReferenceCallbacks";

#[derive(Debug, Clone)]
pub struct ReferenceConfig {
    pub intrusive: bool,
    pub default_key: Option<String>,
    pub frozen: bool,
    pub max_id: i32,
    pub lifecycle: V,
    pub handle: V,
    pub sync: bool,
    pub add_callbacks: Vec<V>,
    pub bake_callbacks: Vec<V>,
    pub clear_callbacks: Vec<V>,
}

#[derive(Debug, Clone)]
pub enum RegistryCallbackInvocation {
    Classify {
        callback: V,
        registry: V,
    },
    Add {
        callback: V,
        registry: V,
        id: i32,
        key: V,
        value: V,
    },
    Bake {
        callback: V,
        registry: V,
    },
}

pub fn execute_callbacks(ctx: &mut ExecCtx<'_>, callbacks: Vec<RegistryCallbackInvocation>) -> V {
    for callback in callbacks {
        let (member, args) = match callback {
            RegistryCallbackInvocation::Classify { callback, registry } => {
                if callback.is_null() { continue; }
                let Some(class) = ctx.rt.class_of(callback) else {
                    return ctx.rt.throw_exception("java/lang/ClassCastException", "Registry callback must be an Object");
                };
                let add = ctx.linked.is_subtype(class, "net/neoforged/neoforge/registries/callback/AddCallback", ctx.host);
                let bake = ctx.linked.is_subtype(class, "net/neoforged/neoforge/registries/callback/BakeCallback", ctx.host);
                let clear = ctx.linked.is_subtype(class, "net/neoforged/neoforge/registries/callback/ClearCallback", ctx.host);
                if !add && !bake && !clear && !ctx.linked.has_class(class)
                    && !ctx.linked.is_subtype(class, "net/neoforged/neoforge/registries/callback/RegistryCallback", ctx.host)
                {
                    return ctx.rt.throw_exception("java/lang/UnsupportedOperationException", "Registry callback class metadata is unavailable");
                }
                (MemberRef::new(CALLBACKS, "accept", "(Lnet/minecraft/core/Registry;Ljava/lang/Object;ZZZ)V"), vec![registry, callback, V::bool(add), V::bool(bake), V::bool(clear)])
            }
            RegistryCallbackInvocation::Add { callback, registry, id, key, value } => (
                MemberRef::new("net/neoforged/neoforge/registries/callback/AddCallback", "onAdd", "(Lnet/minecraft/core/Registry;ILnet/minecraft/resources/ResourceKey;Ljava/lang/Object;)V"),
                vec![callback, registry, V::I(id), key, value],
            ),
            RegistryCallbackInvocation::Bake { callback, registry } => (
                MemberRef::new("net/neoforged/neoforge/registries/callback/BakeCallback", "onBake", "(Lnet/minecraft/core/Registry;)V"),
                vec![callback, registry],
            ),
        };
        invoke_member(
            ctx,
            if member.owner == CALLBACKS {
                op::INVOKESTATIC
            } else {
                op::INVOKEINTERFACE
            },
            &member,
            &args,
        );
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
    }
    V::Void
}

#[derive(Debug, Clone, Default)]
pub struct ReferenceState {
    pub config: Option<ReferenceConfig>,
    pub by_key: BTreeMap<String, V>,
    pub by_value: BTreeMap<u32, V>,
    pub pending: BTreeMap<u32, V>,
    pub registration_infos: BTreeMap<String, V>,
    pub owner: Option<V>,
    pub order: Vec<V>,
    pub frozen: bool,
}

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    (owner == INFO && name == "BUILT_IN" && desc == "Lnet/minecraft/core/RegistrationInfo;")
        .then(|| built_in_registration_info(rt))
}

pub fn built_in_registration_info(rt: &mut Rt) -> V {
    if let Some(info) = rt.get_static(INFO, "BUILT_IN") {
        return info;
    }
    let optional = rt.alloc("java/util/Optional", 0, Data::Optional(None));
    let lifecycle = stable_lifecycle(rt);
    let info = rt.new_instance(INFO, 0);
    rt.put_field(info, "knownPackInfo", optional);
    rt.put_field(info, "lifecycle", lifecycle);
    rt.put_static(INFO, "BUILT_IN", info);
    info
}

fn stable_lifecycle(rt: &mut Rt) -> V {
    if let Some(lifecycle) = rt.get_static("com/mojang/serialization/Lifecycle", "stable:0") {
        return lifecycle;
    }
    let lifecycle = rt.new_pinned_handle(
        "com/mojang/serialization/Lifecycle",
        "dfu_lifecycle",
        0,
        "stable",
    );
    rt.put_static("com/mojang/serialization/Lifecycle", "stable:0", lifecycle);
    lifecycle
}

impl RegistryStore {
    /// The vanilla 1.21.1 BuiltInRegistries initializer creates BLOCK through
    /// registerDefaultedWithIntrusiveHolders(key, "air", bootstrap), using a
    /// stable lifecycle. Exposing this configuration does not materialize the
    /// missing vanilla values or claim complete enumeration/protocol IDs.
    pub fn ensure_builtin_block_registry(&mut self, rt: &mut Rt) -> V {
        const BLOCK_REGISTRY: &str = "minecraft:block";
        if let Some(config) = self
            .reference_states
            .get(BLOCK_REGISTRY)
            .and_then(|state| state.config.as_ref())
        {
            return config.handle;
        }
        let lifecycle = stable_lifecycle(rt);
        let handle = rt.new_pinned_handle(
            "net/minecraft/core/DefaultedMappedRegistry",
            "registry",
            0,
            BLOCK_REGISTRY,
        );
        let state = self.reference_states.entry(BLOCK_REGISTRY.into()).or_default();
        state.config = Some(ReferenceConfig {
            intrusive: true,
            default_key: Some("minecraft:air".into()),
            frozen: self.frozen || state.frozen,
            max_id: i32::MAX - 1,
            lifecycle,
            handle,
            sync: false,
            add_callbacks: Vec::new(),
            bake_callbacks: Vec::new(),
            clear_callbacks: Vec::new(),
        });
        handle
    }

    pub fn take_reference_callbacks(&mut self) -> Vec<RegistryCallbackInvocation> {
        std::mem::take(&mut self.reference_callbacks)
    }
    pub fn reference_owner(&mut self, rt: &mut Rt, registry: &str) -> V {
        let registry = canonical_registry_key(registry);
        let state = self.reference_states.entry(registry.clone()).or_default();
        if let Some(owner) = state.owner {
            return owner;
        }
        let owner = rt.new_pinned_handle(OWNER, "registry_holder_owner", 0, registry);
        state.owner = Some(owner);
        owner
    }

    pub fn canonical_reference(&self, registry: &str, location: &str) -> Option<V> {
        let registry = canonical_registry_key(registry);
        let location = self.resolve_alias(&registry, location);
        self.reference_states
            .get(&registry)?
            .by_key
            .get(&location)
            .copied()
    }

    pub fn create_intrusive_reference(&mut self, rt: &mut Rt, registry: &str, value: V) -> V {
        let registry = canonical_registry_key(registry);
        let Some(config) = self
            .reference_states
            .get(&registry)
            .and_then(|state| state.config.as_ref())
        else {
            return rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("Intrusive holder configuration is unavailable for registry {registry}"),
            );
        };
        if !config.intrusive {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                "This registry can't create intrusive holders",
            );
        }
        if self.frozen
            || config.frozen
            || self
                .reference_states
                .get(&registry)
                .is_some_and(|s| s.frozen)
        {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                "Registry is already frozen",
            );
        }
        if value.is_null() {
            return rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Null-valued intrusive references are not implemented",
            );
        }
        if !matches!(value, V::R(_)) {
            return rt.throw_exception(
                "java/lang/ClassCastException",
                "Registry value must be an Object",
            );
        }
        if let Some(reference) = self
            .reference_states
            .get(&registry)
            .and_then(|state| state.pending.get(&value.r()))
            .copied()
        {
            return reference;
        }
        rt.pin(value);
        let owner = self.reference_owner(rt, &registry);
        let reference = rt.new_instance(REFERENCE, 0);
        rt.put_field(reference, "$owner", owner);
        rt.put_field(reference, "$value", value);
        rt.put_field(reference, "$intrusive", V::TRUE);
        rt.pin(reference);
        self.reference_states
            .entry(registry)
            .or_default()
            .pending
            .insert(value.r(), reference);
        reference
    }

    pub fn register_reference_value(
        &mut self,
        rt: &mut Rt,
        registry: &str,
        location: &str,
        mod_id: &str,
        value: V,
        props: ContentProperties,
        info: V,
    ) -> V {
        if rt.pending_exception.is_some() {
            return V::NULL;
        }
        let registry = canonical_registry_key(registry);
        if self.frozen
            || self
                .reference_states
                .get(&registry)
                .is_some_and(|s| s.frozen)
            || self
                .reference_states
                .get(&registry)
                .and_then(|s| s.config.as_ref())
                .is_some_and(|c| c.frozen)
        {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                "Registry is already frozen",
            );
        }
        if value.is_null() {
            return rt.throw_exception("java/lang/NullPointerException", "registry value");
        }
        if !matches!(value, V::R(_)) {
            return rt.throw_exception(
                "java/lang/ClassCastException",
                "Registry value must be an Object",
            );
        }
        if !valid_location(location) {
            return rt.throw_exception(
                "java/lang/IllegalArgumentException",
                "Invalid registration ResourceLocation",
            );
        }
        if self
            .entries
            .get(&registry)
            .and_then(|entries| entries.get(location))
            .is_some_and(|entry| !entry.value.is_null())
        {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                &format!("Adding duplicate key '{location}' to registry"),
            );
        }
        if self.entries.get(&registry).is_some_and(|entries| {
            entries
                .values()
                .any(|entry| !entry.value.is_null() && entry.value == value)
        }) {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                "Adding duplicate value to registry",
            );
        }
        if rt.class_of(info) != Some(INFO) || rt.get_field(info, "lifecycle").is_null() {
            return rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "RegistrationInfo lifecycle is unavailable",
            );
        }
        let intrusive = self
            .reference_states
            .get(&registry)
            .and_then(|s| s.config.as_ref())
            .is_some_and(|c| c.intrusive);
        let id_count = self
            .reference_states
            .get(&registry)
            .map_or(0, |s| s.order.len());
        if self
            .reference_states
            .get(&registry)
            .and_then(|s| s.config.as_ref())
            .is_some_and(|c| id_count > c.max_id as usize)
        {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                "Registry maximum numeric id exceeded",
            );
        }
        let reference = if intrusive {
            let Some(reference) = self
                .reference_states
                .get(&registry)
                .and_then(|state| state.pending.get(&value.r()))
                .copied()
            else {
                return rt.throw_exception(
                    "java/lang/AssertionError",
                    &format!("Missing intrusive holder for {location}"),
                );
            };
            reference
        } else if let Some(reference) = self
            .reference_states
            .get(&registry)
            .and_then(|state| state.by_key.get(location))
            .copied()
        {
            reference
        } else {
            let owner = self.reference_owner(rt, &registry);
            let reference = rt.new_instance(REFERENCE, 0);
            rt.put_field(reference, "$owner", owner);
            rt.put_field(reference, "$intrusive", V::FALSE);
            rt.pin(reference);
            reference
        };
        let previous_key = rt.get_field(reference, "$key");
        let key = crate::minecraft::new_resource_key(rt, &registry, location);
        if !previous_key.is_null() && previous_key != key {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                "Can't change the key of an existing registry holder",
            );
        }
        if rt.get_field(reference, "$intrusive") == V::TRUE
            && rt.get_field(reference, "$value") != value
        {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                "Can't change the value of an intrusive holder",
            );
        }
        rt.put_field(reference, "$key", key);
        rt.put_field(reference, "$value", value);
        rt.pin(value);
        rt.pin(info);
        let state = self.reference_states.entry(registry.clone()).or_default();
        state.by_key.insert(location.into(), reference);
        state.by_value.insert(value.r(), reference);
        state.pending.remove(&value.r());
        state.order.push(reference);
        state.registration_infos.insert(location.into(), info);
        if let Some(config) = state.config.as_ref() {
            self.reference_callbacks
                .extend(config.add_callbacks.iter().map(|callback| {
                    RegistryCallbackInvocation::Add {
                        callback: *callback,
                        registry: config.handle,
                        id: id_count as i32,
                        key,
                        value,
                    }
                }));
        }
        self.entries.entry(registry.clone()).or_default().insert(
            location.into(),
            RegisteredEntry {
                registry: registry.clone(),
                id: location.into(),
                mod_id: mod_id.into(),
                value,
                properties: props,
            },
        );
        for (holder_registry, holder_location, slot) in self.holders.values_mut() {
            if holder_registry == &registry && holder_location == location {
                *slot = Some(value);
            }
        }
        reference
    }

    pub fn freeze_reference_registry(&mut self, rt: &mut Rt, registry: &str) -> V {
        let registry = canonical_registry_key(registry);
        let state = self.reference_states.entry(registry).or_default();
        if state.frozen || state.config.as_ref().is_some_and(|config| config.frozen) {
            return V::Void;
        }
        state.frozen = true;
        if let Some(config) = state.config.as_mut() {
            config.frozen = true;
        }
        if !state.pending.is_empty() {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                "Some intrusive holders were not registered",
            );
        }
        if state.by_key.values().any(|reference| {
            rt.get_field(*reference, "$key").is_null()
                || rt.get_field(*reference, "$value").is_null()
        }) {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                "Unbound values in registry",
            );
        }
        if let Some(config) = state.config.as_ref() {
            self.reference_callbacks
                .extend(config.bake_callbacks.iter().map(|callback| {
                    RegistryCallbackInvocation::Bake {
                        callback: *callback,
                        registry: config.handle,
                    }
                }));
        }
        V::Void
    }

    pub fn freeze_checked(&mut self, rt: &mut Rt) -> V {
        let keys: Vec<_> = self.reference_states.keys().cloned().collect();
        for registry in keys {
            self.freeze_reference_registry(rt, &registry);
            if rt.pending_exception.is_some() {
                return V::NULL;
            }
        }
        self.frozen = true;
        V::Void
    }
}

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == "net/minecraft/core/DefaultedMappedRegistry"
        && matches!(
            sup,
            REGISTRY
                | "net/minecraft/core/MappedRegistry"
                | "net/minecraft/core/DefaultedRegistry"
                | "net/minecraft/core/WritableRegistry"
        ))
        || (sub == INFO && sup == "java/lang/Record")
        || (matches!(
            sub,
            "net/neoforged/neoforge/registries/callback/AddCallback"
                | "net/neoforged/neoforge/registries/callback/BakeCallback"
                | "net/neoforged/neoforge/registries/callback/ClearCallback"
        ) && sup == "net/neoforged/neoforge/registries/callback/RegistryCallback")
}

pub fn call(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if is_static
        && owner == CALLBACKS
        && name == "accept"
        && desc == "(Lnet/minecraft/core/Registry;Ljava/lang/Object;ZZZ)V"
    {
        let reg = registry_name(rt, *args.first()?)?;
        let callback = *args.get(1)?;
        let config = registries.reference_states.get_mut(&reg)?.config.as_mut()?;
        if args.get(2)? == &V::TRUE {
            config.add_callbacks.push(callback);
        }
        if args.get(3)? == &V::TRUE {
            config.bake_callbacks.push(callback);
        }
        if args.get(4)? == &V::TRUE {
            config.clear_callbacks.push(callback);
        }
        rt.pin(callback);
        return Some(V::Void);
    }
    if owner == INFO && !is_static {
        let recv = *args.first()?;
        return match (name, desc) {
            ("<init>", "(Ljava/util/Optional;Lcom/mojang/serialization/Lifecycle;)V") => {
                rt.put_field(recv, "knownPackInfo", *args.get(1)?);
                rt.put_field(recv, "lifecycle", *args.get(2)?);
                Some(V::Void)
            }
            ("knownPackInfo", "()Ljava/util/Optional;") => {
                Some(rt.get_field(recv, "knownPackInfo"))
            }
            ("lifecycle", "()Lcom/mojang/serialization/Lifecycle;") => {
                Some(rt.get_field(recv, "lifecycle"))
            }
            _ => None,
        };
    }
    if owner == BUILDER && !is_static {
        return builder_call(rt, registries, member, args);
    }
    if matches!(
        owner,
        REGISTRY
            | "net/minecraft/core/DefaultedRegistry"
            | "net/minecraft/core/MappedRegistry"
            | "net/minecraft/core/DefaultedMappedRegistry"
            | "net/minecraft/core/WritableRegistry"
            | "net/neoforged/neoforge/registries/BaseMappedRegistry"
    ) {
        if is_static {
            if owner != REGISTRY
                || !matches!(name, "register" | "registerForHolder")
                || args.len() != 3
            {
                return None;
            }
            let known = match name {
                "register" => matches!(desc,
                    "(Lnet/minecraft/core/Registry;Ljava/lang/String;Ljava/lang/Object;)Ljava/lang/Object;"
                    | "(Lnet/minecraft/core/Registry;Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Object;)Ljava/lang/Object;"
                    | "(Lnet/minecraft/core/Registry;Lnet/minecraft/resources/ResourceKey;Ljava/lang/Object;)Ljava/lang/Object;"),
                _ => matches!(desc,
                    "(Lnet/minecraft/core/Registry;Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;"
                    | "(Lnet/minecraft/core/Registry;Lnet/minecraft/resources/ResourceKey;Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;"),
            };
            if !known {
                return None;
            }
            let reg = registry_name(rt, *args.first()?)?;
            let location = match registration_location(rt, &reg, *args.get(1)?) {
                Ok(location) => location,
                Err(_) => return Some(V::NULL),
            };
            let value = *args.get(2)?;
            let info = built_in_registration_info(rt);
            let namespace = location
                .split_once(':')
                .map_or("minecraft", |(namespace, _)| namespace);
            let reference = registries.register_reference_value(
                rt,
                &reg,
                &location,
                namespace,
                value,
                ContentProperties::default(),
                info,
            );
            return Some(
                if name == "registerForHolder" || rt.pending_exception.is_some() {
                    reference
                } else {
                    value
                },
            );
        }
        let registry = *args.first()?;
        let reg = registry_name(rt, registry)?;
        if name == "createIntrusiveHolder"
            && desc == "(Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;"
        {
            return Some(registries.create_intrusive_reference(rt, &reg, *args.get(1)?));
        }
        if name == "register" && desc == "(Lnet/minecraft/resources/ResourceKey;Ljava/lang/Object;Lnet/minecraft/core/RegistrationInfo;)Lnet/minecraft/core/Holder$Reference;" {
            let location = match registration_location(rt, &reg, *args.get(1)?) { Ok(location) => location, Err(_) => return Some(V::NULL) };
            let namespace = location.split_once(':').map_or("minecraft", |(namespace, _)| namespace);
            return Some(registries.register_reference_value(rt, &reg, &location, namespace, *args.get(2)?, ContentProperties::default(), *args.get(3)?));
        }
        if name == "getHolder"
            && matches!(
                desc,
                "(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/Optional;"
                    | "(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;"
            )
        {
            let location = match registration_location(rt, &reg, *args.get(1)?) {
                Ok(location) => location,
                Err(_) => return Some(V::NULL),
            };
            let reference = registries.canonical_reference(&reg, &location);
            if reference.is_none()
                && (reg.starts_with("minecraft:") || reg.starts_with("neoforge:"))
                && (location.starts_with("minecraft:") || location.starts_with("neoforge:"))
            {
                return Some(rt.throw_exception("java/lang/UnsupportedOperationException", &format!("getHolder cannot determine missing definition {location} in incomplete built-in registry {reg}")));
            }
            return Some(rt.alloc("java/util/Optional", 0, Data::Optional(reference)));
        }
        if name == "registrationInfo"
            && desc == "(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;"
        {
            let location = match registration_location(rt, &reg, *args.get(1)?) {
                Ok(location) => location,
                Err(_) => return Some(V::NULL),
            };
            let location = registries.resolve_alias(&reg, &location);
            let info = registries
                .reference_states
                .get(&reg)
                .and_then(|state| state.registration_infos.get(&location))
                .copied();
            return Some(rt.alloc("java/util/Optional", 0, Data::Optional(info)));
        }
        if name == "holderOwner" && desc == "()Lnet/minecraft/core/HolderOwner;" {
            return Some(registries.reference_owner(rt, &reg));
        }
        if name == "wrapAsHolder" && desc == "(Ljava/lang/Object;)Lnet/minecraft/core/Holder;" {
            let value = *args.get(1)?;
            if let Some(reference) = registries
                .reference_states
                .get(&reg)
                .and_then(|state| state.by_value.get(&value.r()))
                .copied()
            {
                return Some(reference);
            }
            let direct = rt.new_instance("net/minecraft/core/Holder$Direct", 0);
            rt.put_field(direct, "$value", value);
            return Some(direct);
        }
        if name == "freeze" && desc == "()Lnet/minecraft/core/Registry;" {
            registries.freeze_reference_registry(rt, &reg);
            return Some(if rt.pending_exception.is_some() {
                V::NULL
            } else {
                registry
            });
        }
        if name == "unfreeze" && desc == "()V" {
            let state = registries.reference_states.entry(reg.clone()).or_default();
            state.frozen = false;
            if let Some(config) = state.config.as_mut() {
                config.frozen = false;
            }
            return Some(V::Void);
        }
        if name == "addAlias" && desc == "(Lnet/minecraft/resources/ResourceLocation;Lnet/minecraft/resources/ResourceLocation;)V" {
            let from = match registration_location(rt, &reg, *args.get(1)?) { Ok(location) => location, Err(_) => return Some(V::NULL) };
            let to = match registration_location(rt, &reg, *args.get(2)?) { Ok(location) => location, Err(_) => return Some(V::NULL) };
            return Some(match registries.add_alias(&reg, &from, &to) { Ok(_) => V::Void, Err(error) => rt.throw_exception("java/lang/IllegalArgumentException", &error) });
        }
        if name == "resolve" && desc == "(Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/resources/ResourceLocation;" {
            let original = *args.get(1)?;
            let location = registration_location(rt, &reg, original).ok()?;
            let resolved = registries.resolve_alias(&reg, &location);
            return Some(if resolved == location { original } else { resource_location(rt, &resolved) });
        }
        if name == "holders" && desc == "()Ljava/util/stream/Stream;" {
            if !registries.custom_registries.contains(&reg) {
                return Some(rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "Complete built-in holder enumeration is unavailable",
                ));
            }
            let values = registries
                .reference_states
                .get(&reg)
                .map_or_else(Vec::new, |state| state.order.clone());
            return Some(rt.alloc("java/util/stream/Stream", 0, Data::Stream(values)));
        }
        if name == "getDefaultKey" && desc == "()Lnet/minecraft/resources/ResourceLocation;" {
            let default = registries
                .reference_states
                .get(&reg)
                .and_then(|state| state.config.as_ref())
                .and_then(|config| config.default_key.as_ref())?
                .clone();
            return Some(resource_location(rt, &default));
        }
        if name == "get"
            && matches!(
                desc,
                "(Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;"
                    | "(Lnet/minecraft/resources/ResourceKey;)Ljava/lang/Object;"
            )
        {
            let location = match registration_location(rt, &reg, *args.get(1)?) {
                Ok(location) => location,
                Err(_) => return Some(V::NULL),
            };
            if let Some(value) = registries
                .lookup(&reg, &location)
                .map(|entry| entry.value)
                .filter(|value| !value.is_null())
            {
                return Some(value);
            }
            if let Some(default) = registries
                .reference_states
                .get(&reg)
                .and_then(|state| state.config.as_ref())
                .and_then(|config| config.default_key.as_ref())
            {
                if let Some(value) = registries
                    .lookup(&reg, default)
                    .map(|entry| entry.value)
                    .filter(|value| !value.is_null())
                {
                    return Some(value);
                }
                return Some(rt.throw_exception(
                    "java/lang/NullPointerException",
                    "Default registry entry is not registered",
                ));
            }
            if (reg.starts_with("minecraft:") || reg.starts_with("neoforge:"))
                && (location.starts_with("minecraft:") || location.starts_with("neoforge:"))
            {
                return Some(rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "Built-in registry definition is incomplete",
                ));
            }
            return Some(V::NULL);
        }
        if name == "getKey"
            && desc == "(Ljava/lang/Object;)Lnet/minecraft/resources/ResourceLocation;"
        {
            let value = *args.get(1)?;
            let location = registries
                .entries
                .get(&reg)
                .and_then(|entries| {
                    entries
                        .values()
                        .find(|entry| !entry.value.is_null() && entry.value == value)
                })
                .map(|entry| entry.id.clone())
                .or_else(|| {
                    registries
                        .reference_states
                        .get(&reg)
                        .and_then(|state| state.config.as_ref())
                        .and_then(|config| config.default_key.clone())
                });
            return Some(location.map_or(V::NULL, |location| resource_location(rt, &location)));
        }
        return None;
    }
    if owner == OWNER
        && !is_static
        && name == "canSerializeIn"
        && desc == "(Lnet/minecraft/core/HolderOwner;)Z"
    {
        return Some(V::bool(args.first()? == args.get(1)?));
    }
    if matches!(
        owner,
        "net/minecraft/core/Holder" | REFERENCE | "net/minecraft/core/Holder$Direct"
    ) && !is_static
    {
        let recv = *args.first()?;
        if rt.class_of(recv) == Some("net/minecraft/core/Holder$Direct") {
            return match (name, desc) {
                ("value", "()Ljava/lang/Object;") => Some(rt.get_field(recv, "$value")),
                ("isBound", "()Z") => Some(V::TRUE),
                ("unwrapKey", "()Ljava/util/Optional;") => {
                    Some(rt.alloc("java/util/Optional", 0, Data::Optional(None)))
                }
                _ => None,
            };
        }
        let reference_owner = rt.get_field(recv, "$owner");
        if !rt
            .handle_of(reference_owner)
            .is_some_and(|(kind, ..)| kind == "registry_holder_owner")
        {
            return None;
        }
        let key = rt.get_field(recv, "$key");
        let value = rt.get_field(recv, "$value");
        return match (name, desc) {
            ("value" | "get", "()Ljava/lang/Object;") => Some(if value.is_null() {
                rt.throw_exception(
                    "java/lang/IllegalStateException",
                    "Trying to access unbound value",
                )
            } else {
                value
            }),
            ("key", "()Lnet/minecraft/resources/ResourceKey;") => Some(if key.is_null() {
                rt.throw_exception(
                    "java/lang/IllegalStateException",
                    "Trying to access unbound key",
                )
            } else {
                key
            }),
            ("isBound", "()Z") => Some(V::bool(!key.is_null() && !value.is_null())),
            ("unwrapKey", "()Ljava/util/Optional;") => Some(if key.is_null() {
                rt.throw_exception(
                    "java/lang/IllegalStateException",
                    "Trying to access unbound key",
                )
            } else {
                rt.alloc("java/util/Optional", 0, Data::Optional(Some(key)))
            }),
            ("canSerializeIn", "(Lnet/minecraft/core/HolderOwner;)Z") => {
                Some(V::bool(reference_owner == *args.get(1)?))
            }
            ("is", "(Lnet/minecraft/resources/ResourceKey;)Z") => Some(if key.is_null() {
                rt.throw_exception(
                    "java/lang/IllegalStateException",
                    "Trying to access unbound key",
                )
            } else {
                V::bool(key == *args.get(1)?)
            }),
            ("is", "(Lnet/minecraft/resources/ResourceLocation;)Z") => Some(if key.is_null() {
                rt.throw_exception(
                    "java/lang/IllegalStateException",
                    "Trying to access unbound key",
                )
            } else {
                let (_, location) = crate::minecraft::resource_key_parts(rt, key)?;
                V::bool(location_from_value(rt, *args.get(1)?)? == location)
            }),
            _ => None,
        };
    }
    None
}

fn builder_call(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    member: &MemberRef,
    args: &[V],
) -> Option<V> {
    let recv = *args.first()?;
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if name == "<init>" && desc == "(Lnet/minecraft/resources/ResourceKey;)V" {
        let key = *args.get(1)?;
        let Some((root, registry)) = crate::minecraft::resource_key_parts(rt, key) else {
            return Some(
                rt.throw_exception("java/lang/NullPointerException", "registry builder key"),
            );
        };
        if root != "minecraft:root" || !valid_location(&registry) {
            return Some(rt.throw_exception(
                "java/lang/IllegalArgumentException",
                "Builder requires a registry ResourceKey",
            ));
        }
        rt.put_field(recv, "$registryKey", key);
        rt.put_field(recv, "$intrusive", V::FALSE);
        rt.put_field(recv, "$sync", V::FALSE);
        rt.put_field(recv, "$maxId", V::I(-1));
        rt.put_field(recv, "$registrationCheck", V::TRUE);
        let callbacks = rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new()));
        rt.put_field(recv, "$callbacks", callbacks);
        return Some(V::Void);
    }
    if name == "withIntrusiveHolders"
        && desc == "()Lnet/neoforged/neoforge/registries/RegistryBuilder;"
    {
        rt.put_field(recv, "$intrusive", V::TRUE);
        return Some(recv);
    }
    if name == "sync" && desc == "(Z)Lnet/neoforged/neoforge/registries/RegistryBuilder;" {
        rt.put_field(recv, "$sync", *args.get(1)?);
        return Some(recv);
    }
    if name == "disableRegistrationCheck"
        && desc == "()Lnet/neoforged/neoforge/registries/RegistryBuilder;"
    {
        rt.put_field(recv, "$registrationCheck", V::FALSE);
        return Some(recv);
    }
    if name == "defaultKey" && matches!(desc, "(Lnet/minecraft/resources/ResourceLocation;)Lnet/neoforged/neoforge/registries/RegistryBuilder;" | "(Lnet/minecraft/resources/ResourceKey;)Lnet/neoforged/neoforge/registries/RegistryBuilder;") {
        let key = *args.get(1)?;
        let location = if desc.starts_with("(Lnet/minecraft/resources/ResourceKey;") { if key.is_null() { return Some(rt.throw_exception("java/lang/NullPointerException", "default resource key")); } let (_, location) = crate::minecraft::resource_key_parts(rt, key)?; resource_location(rt, &location) } else { key };
        rt.put_field(recv, "$defaultKey", location); return Some(recv);
    }
    if name == "maxId" && desc == "(I)Lnet/neoforged/neoforge/registries/RegistryBuilder;" {
        let maximum = args.get(1)?.i();
        if maximum < 0 {
            return Some(rt.throw_exception(
                "java/lang/IllegalArgumentException",
                "maxId must be greater than or equal to zero",
            ));
        }
        rt.put_field(recv, "$maxId", V::I(maximum));
        return Some(recv);
    }
    if matches!(name, "callback" | "onAdd" | "onBake" | "onClear") {
        let callback = *args.get(1)?;
        let callbacks = rt.get_field(recv, "$callbacks");
        if let Some(Data::List(values)) = rt.get_obj_mut(callbacks).map(|o| &mut o.data) {
            values.push(callback);
            rt.pin(callback);
            return Some(recv);
        }
        return None;
    }
    if name == "create" && desc == "()Lnet/minecraft/core/Registry;" {
        let key = rt.get_field(recv, "$registryKey");
        let (root, registry) = crate::minecraft::resource_key_parts(rt, key)?;
        if root != "minecraft:root" {
            return Some(rt.throw_exception(
                "java/lang/IllegalArgumentException",
                "Builder requires registry key",
            ));
        }
        if registries
            .reference_states
            .get(&registry)
            .is_some_and(|state| state.config.is_some())
        {
            return Some(rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Multiple registry instances with the same key are not implemented",
            ));
        }
        let default_key = rt.get_field(recv, "$defaultKey");
        let default_key = if default_key.is_null() {
            None
        } else {
            Some(location_from_value(rt, default_key)?)
        };
        let maximum = rt.get_field(recv, "$maxId").i();
        let lifecycle = stable_lifecycle(rt);
        let class = if default_key.is_some() {
            "net/minecraft/core/DefaultedMappedRegistry"
        } else {
            "net/minecraft/core/MappedRegistry"
        };
        let handle = rt.new_pinned_handle(class, "registry", 0, &registry);
        let callbacks = match rt
            .get_obj(rt.get_field(recv, "$callbacks"))
            .map(|o| &o.data)
        {
            Some(Data::List(values)) => values.clone(),
            _ => return None,
        };
        registries
            .reference_states
            .entry(registry.clone())
            .or_default()
            .config = Some(ReferenceConfig {
            intrusive: rt.get_field(recv, "$intrusive") == V::TRUE,
            default_key: default_key.clone(),
            frozen: false,
            max_id: if maximum < 0 { i32::MAX - 1 } else { maximum },
            lifecycle,
            handle,
            sync: rt.get_field(recv, "$sync") == V::TRUE,
            add_callbacks: Vec::new(),
            bake_callbacks: Vec::new(),
            clear_callbacks: Vec::new(),
        });
        registries.entries.entry(registry.clone()).or_default();
        if !registries.custom_registries.contains(&registry) {
            registries.custom_registries.push(registry.clone());
        }
        registries
            .reference_callbacks
            .extend(
                callbacks
                    .into_iter()
                    .map(|callback| RegistryCallbackInvocation::Classify {
                        callback,
                        registry: handle,
                    }),
            );
        return Some(handle);
    }
    None
}

fn registry_name(rt: &Rt, registry: V) -> Option<String> {
    let ("registry", _, registry, _) = rt.handle_of(registry)? else {
        return None;
    };
    Some(registry.into())
}
fn location_from_value(rt: &Rt, value: V) -> Option<String> {
    let ("resource_location", _, location, _) = rt.handle_of(value)? else {
        return None;
    };
    Some(location.into())
}
fn resource_location(rt: &mut Rt, location: &str) -> V {
    rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        location,
    )
}
fn registration_location(rt: &mut Rt, registry: &str, key: V) -> Result<String, ()> {
    if key.is_null() {
        rt.throw_exception("java/lang/NullPointerException", "registry key");
        return Err(());
    }
    let location = if let Some((owner, location)) = crate::minecraft::resource_key_parts(rt, key) {
        if owner != registry {
            rt.throw_exception(
                "java/lang/IllegalArgumentException",
                "ResourceKey belongs to a different registry",
            );
            return Err(());
        }
        location
    } else if let Some(location) = location_from_value(rt, key) {
        location
    } else if let Some(location) = rt.str_of(key) {
        if location.contains(':') {
            location.into()
        } else {
            format!("minecraft:{location}")
        }
    } else {
        rt.throw_exception(
            "java/lang/ClassCastException",
            "registry key must be ResourceKey, ResourceLocation, or String",
        );
        return Err(());
    };
    if !valid_location(&location) {
        rt.throw_exception(
            "java/lang/IllegalArgumentException",
            "Invalid registration ResourceLocation",
        );
        return Err(());
    }
    Ok(location)
}

fn valid_location(location: &str) -> bool {
    let Some((namespace, path)) = location.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && namespace
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
        && path
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"/_.-".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, ExecCtx, HostApi, LinkedSpace};
    struct ReferenceHost(RegistryStore);
    impl HostApi for ReferenceHost {
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
            if let Some(value) = super::call(rt, &mut self.0, member, is_static, args) {
                loop {
                    let callbacks = self.0.take_reference_callbacks();
                    if callbacks.is_empty() {
                        break;
                    }
                    super::execute_callbacks(
                        &mut ExecCtx {
                            rt,
                            linked,
                            host: self,
                            depth: 0,
                        },
                        callbacks,
                    );
                    if rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                }
                return Some(value);
            }
            crate::registry_codecs::call(
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
            .or_else(|| crate::dfu::call(rt, linked, self, member, is_static, args))
            .or_else(|| {
                crate::minecraft::call_keys_authlib_and_libs(
                    rt,
                    &mut self.0,
                    member,
                    is_static,
                    args,
                )
            })
        }
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            super::is_subtype(sub, sup) || crate::minecraft::is_subtype(sub, sup)
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
    fn registry(ctx: &mut ExecCtx<'_>, name: &str, intrusive: bool) -> V {
        let key = crate::minecraft::new_resource_key(ctx.rt, "minecraft:root", name);
        let builder = ctx.rt.new_instance(BUILDER, 0);
        invoke_member(
            ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(
                BUILDER,
                "<init>",
                "(Lnet/minecraft/resources/ResourceKey;)V",
            ),
            &[builder, key],
        );
        if intrusive {
            invoke(
                ctx,
                BUILDER,
                "withIntrusiveHolders",
                "()Lnet/neoforged/neoforge/registries/RegistryBuilder;",
                &[builder],
            );
        }
        let registry = invoke(
            ctx,
            BUILDER,
            "create",
            "()Lnet/minecraft/core/Registry;",
            &[builder],
        );
        assert!(ctx.rt.pending_exception.is_none());
        registry
    }
    fn location(rt: &mut Rt, name: &str) -> V {
        rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            name,
        )
    }
    fn register(ctx: &mut ExecCtx<'_>, registry: V, key: V, value: V) -> V {
        invoke_member(ctx, op::INVOKESTATIC, &MemberRef::new(REGISTRY, "registerForHolder", "(Lnet/minecraft/core/Registry;Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;"), &[registry, key, value])
    }
    fn optional(ctx: &ExecCtx<'_>, value: V) -> Option<V> {
        let Data::Optional(value) = &ctx.rt.get_obj(value).unwrap().data else {
            panic!("expected Optional")
        };
        *value
    }
    fn expect_exception(ctx: &mut ExecCtx<'_>, expected: &str) {
        let error = ctx.rt.take_exception().expect("expected exception");
        let summary = ctx.rt.exception_summary(error);
        assert!(summary.contains(expected), "expected {expected}: {summary}");
    }

    #[test]
    fn defaulted_registry_codecs_never_encode_unregistered_values_as_the_default_entry() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ReferenceHost(RegistryStore::new());
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let key = crate::minecraft::new_resource_key(ctx.rt, "minecraft:root", "test:defaulted");
        let default = location(ctx.rt, "test:empty");
        let builder = ctx.rt.new_instance(BUILDER, 0);
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(
                BUILDER,
                "<init>",
                "(Lnet/minecraft/resources/ResourceKey;)V",
            ),
            &[builder, key],
        );
        invoke(&mut ctx, BUILDER, "defaultKey", "(Lnet/minecraft/resources/ResourceLocation;)Lnet/neoforged/neoforge/registries/RegistryBuilder;", &[builder, default]);
        let reg = invoke(
            &mut ctx,
            BUILDER,
            "create",
            "()Lnet/minecraft/core/Registry;",
            &[builder],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let empty = ctx.rt.new_instance("test/Empty", 0);
        let reference = register(&mut ctx, reg, default, empty);
        let missing = location(ctx.rt, "test:missing");
        assert_eq!(
            invoke(
                &mut ctx,
                REGISTRY,
                "get",
                "(Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;",
                &[reg, missing]
            ),
            empty
        );
        let found = invoke(
            &mut ctx,
            REGISTRY,
            "getHolder",
            "(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/Optional;",
            &[reg, missing],
        );
        assert_eq!(optional(&ctx, found), None);
        let codec = invoke(
            &mut ctx,
            REGISTRY,
            "byNameCodec",
            "()Lcom/mojang/serialization/Codec;",
            &[reg],
        );
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let unknown = ctx.rt.new_instance("test/Unregistered", 0);
        let encoded = invoke(&mut ctx, "com/mojang/serialization/Codec", "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, unknown]);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            invoke(
                &mut ctx,
                "com/mojang/serialization/DataResult",
                "isError",
                "()Z",
                &[encoded]
            ),
            V::TRUE
        );
        let ref_codec = invoke(
            &mut ctx,
            REGISTRY,
            "holderByNameCodec",
            "()Lcom/mojang/serialization/Codec;",
            &[reg],
        );
        let foreign = registry(&mut ctx, "test:foreign", false);
        let foreign_value = ctx.rt.new_instance("test/Foreign", 0);
        let foreign_id = location(ctx.rt, "test:foreign");
        let foreign_ref = register(&mut ctx, foreign, foreign_id, foreign_value);
        let encoded = invoke(&mut ctx, "com/mojang/serialization/Codec", "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[ref_codec, ops, foreign_ref]);
        assert_eq!(
            invoke(
                &mut ctx,
                "com/mojang/serialization/DataResult",
                "isError",
                "()Z",
                &[encoded]
            ),
            V::TRUE
        );
        let encoded = invoke(&mut ctx, "com/mojang/serialization/Codec", "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[ref_codec, ops, reference]);
        assert_eq!(
            invoke(
                &mut ctx,
                "com/mojang/serialization/DataResult",
                "isSuccess",
                "()Z",
                &[encoded]
            ),
            V::TRUE
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn registering_an_alias_name_shadows_the_alias_without_rebinding_its_target_reference() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ReferenceHost(RegistryStore::new());
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let reg = registry(&mut ctx, "test:types", false);
        let original = ctx.rt.new_instance("test/Original", 0);
        let id = location(ctx.rt, "test:original");
        let reference = register(&mut ctx, reg, id, original);
        let alias = location(ctx.rt, "test:alias");
        invoke(&mut ctx, REGISTRY, "addAlias", "(Lnet/minecraft/resources/ResourceLocation;Lnet/minecraft/resources/ResourceLocation;)V", &[reg, alias, id]);
        let shadow = ctx.rt.new_instance("test/Shadow", 0);
        let shadow_ref = register(&mut ctx, reg, alias, shadow);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "{:?}",
            ctx.rt.pending_exception
        );
        assert_ne!(shadow_ref, reference);
        let found = invoke(
            &mut ctx,
            REGISTRY,
            "getHolder",
            "(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/Optional;",
            &[reg, alias],
        );
        assert_eq!(optional(&ctx, found), Some(shadow_ref));
        assert_eq!(
            invoke(
                &mut ctx,
                REFERENCE,
                "value",
                "()Ljava/lang/Object;",
                &[reference]
            ),
            original
        );
        assert_eq!(
            invoke(
                &mut ctx,
                REFERENCE,
                "value",
                "()Ljava/lang/Object;",
                &[shadow_ref]
            ),
            shadow
        );
    }

    fn on_add(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let calls = ctx.rt.get_field(args[0], "addCalls").i() + 1;
        ctx.rt.put_field(args[0], "addCalls", V::I(calls));
        ctx.rt.put_field(args[0], "registry", args[1]);
        ctx.rt.put_field(args[0], "id", args[2]);
        ctx.rt.put_field(args[0], "key", args[3]);
        ctx.rt.put_field(args[0], "value", args[4]);
        if ctx.rt.get_field(args[0], "throwAdd") == V::TRUE {
            return ctx
                .rt
                .throw_exception("test/AddCallbackFailure", "after binding");
        }
        V::Void
    }
    fn on_bake(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let calls = ctx.rt.get_field(args[0], "bakeCalls").i() + 1;
        ctx.rt.put_field(args[0], "bakeCalls", V::I(calls));
        ctx.rt.put_field(args[0], "registry", args[1]);
        if ctx.rt.get_field(args[0], "throwBake") == V::TRUE {
            return ctx
                .rt
                .throw_exception("test/BakeCallbackFailure", "after freeze");
        }
        V::Void
    }
    fn callback_linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(mmga_runtime::NativeModuleRegistry {
            classes: &[mmga_runtime::NativeClassDef { name: "test/Callbacks", superclass: Some("java/lang/Object"), interfaces: &["net/neoforged/neoforge/registries/callback/AddCallback", "net/neoforged/neoforge/registries/callback/BakeCallback"], is_interface: false, methods: &[
                ("onAdd.(Lnet/minecraft/core/Registry;ILnet/minecraft/resources/ResourceKey;Ljava/lang/Object;)V", on_add, false, false),
                ("onBake.(Lnet/minecraft/core/Registry;)V", on_bake, false, false),
            ] }], lambdas: &[],
        });
        linked
    }
    fn callback_registry(ctx: &mut ExecCtx<'_>, name: &str, callbacks: &[V]) -> V {
        let key = crate::minecraft::new_resource_key(ctx.rt, "minecraft:root", name);
        let builder = ctx.rt.new_instance(BUILDER, 0);
        invoke_member(
            ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(
                BUILDER,
                "<init>",
                "(Lnet/minecraft/resources/ResourceKey;)V",
            ),
            &[builder, key],
        );
        for callback in callbacks {
            invoke(ctx, BUILDER, "callback", "(Lnet/neoforged/neoforge/registries/callback/RegistryCallback;)Lnet/neoforged/neoforge/registries/RegistryBuilder;", &[builder, *callback]);
        }
        invoke(
            ctx,
            BUILDER,
            "create",
            "()Lnet/minecraft/core/Registry;",
            &[builder],
        )
    }
    #[test]
    fn callback_classification_retains_both_interfaces_and_runs_only_at_actual_registration_and_bake(
    ) {
        let mut rt = Rt::new();
        let linked = callback_linked();
        let mut host = ReferenceHost(RegistryStore::new());
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let callback = ctx.rt.new_instance("test/Callbacks", 0);
        let reg = callback_registry(&mut ctx, "test:callbacks", &[callback, V::NULL]);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(callback, "addCalls").i(), 0);
        assert_eq!(ctx.rt.get_field(callback, "bakeCalls").i(), 0);
        let value = ctx.rt.new_instance("test/Value", 0);
        let id = location(ctx.rt, "test:registered");
        let reference = register(&mut ctx, reg, id, value);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(callback, "addCalls"), V::I(1));
        assert_eq!(ctx.rt.get_field(callback, "bakeCalls").i(), 0);
        assert_eq!(ctx.rt.get_field(callback, "registry"), reg);
        assert_eq!(ctx.rt.get_field(callback, "value"), value);
        assert_eq!(ctx.rt.get_field(callback, "id"), V::I(0));
        assert_eq!(
            ctx.rt.get_field(callback, "key"),
            ctx.rt.get_field(reference, "$key")
        );
        assert_eq!(
            invoke(
                &mut ctx,
                REGISTRY,
                "freeze",
                "()Lnet/minecraft/core/Registry;",
                &[reg]
            ),
            reg
        );
        assert_eq!(ctx.rt.get_field(callback, "bakeCalls"), V::I(1));
        assert_eq!(
            invoke(
                &mut ctx,
                REGISTRY,
                "freeze",
                "()Lnet/minecraft/core/Registry;",
                &[reg]
            ),
            reg
        );
        assert_eq!(ctx.rt.get_field(callback, "bakeCalls"), V::I(1));
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn callback_errors_propagate_synchronously_preserve_bound_values_and_stop_later_callbacks() {
        let mut rt = Rt::new();
        let linked = callback_linked();
        let mut host = ReferenceHost(RegistryStore::new());
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let first = ctx.rt.new_instance("test/Callbacks", 0);
        let second = ctx.rt.new_instance("test/Callbacks", 0);
        ctx.rt.put_field(first, "throwAdd", V::TRUE);
        let reg = callback_registry(&mut ctx, "test:callbacks", &[first, second]);
        assert!(ctx.rt.pending_exception.is_none());
        let value = ctx.rt.new_instance("test/Value", 0);
        let id = location(ctx.rt, "test:registered");
        register(&mut ctx, reg, id, value);
        expect_exception(&mut ctx, "AddCallbackFailure");
        assert_eq!(ctx.rt.get_field(first, "addCalls"), V::I(1));
        assert_eq!(ctx.rt.get_field(second, "addCalls").i(), 0);
        let found = invoke(
            &mut ctx,
            REGISTRY,
            "getHolder",
            "(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/Optional;",
            &[reg, id],
        );
        let reference = optional(&ctx, found).unwrap();
        assert_eq!(
            invoke(
                &mut ctx,
                REFERENCE,
                "value",
                "()Ljava/lang/Object;",
                &[reference]
            ),
            value
        );
        ctx.rt.put_field(first, "throwBake", V::TRUE);
        invoke(
            &mut ctx,
            REGISTRY,
            "freeze",
            "()Lnet/minecraft/core/Registry;",
            &[reg],
        );
        expect_exception(&mut ctx, "BakeCallbackFailure");
        assert_eq!(ctx.rt.get_field(first, "bakeCalls"), V::I(1));
        assert_eq!(ctx.rt.get_field(second, "bakeCalls").i(), 0);
        let late = ctx.rt.new_instance("test/Value", 0);
        let late_id = location(ctx.rt, "test:late");
        register(&mut ctx, reg, late_id, late);
        expect_exception(&mut ctx, "frozen");
    }

    #[test]
    fn intrusive_prereference_identity_is_bound_by_real_registration_and_roundtrips_the_holder_codec(
    ) {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ReferenceHost(RegistryStore::new());
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let reg = registry(&mut ctx, "test:chemical", true);
        let value = ctx.rt.new_instance("test/Chemical", 0);
        let reference = invoke(
            &mut ctx,
            REGISTRY,
            "createIntrusiveHolder",
            "(Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;",
            &[reg, value],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            invoke(
                &mut ctx,
                REGISTRY,
                "createIntrusiveHolder",
                "(Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;",
                &[reg, value]
            ),
            reference
        );
        assert_eq!(
            invoke(
                &mut ctx,
                REFERENCE,
                "value",
                "()Ljava/lang/Object;",
                &[reference]
            ),
            value
        );
        assert_eq!(
            invoke(&mut ctx, REFERENCE, "isBound", "()Z", &[reference]),
            V::FALSE
        );
        invoke(
            &mut ctx,
            REFERENCE,
            "key",
            "()Lnet/minecraft/resources/ResourceKey;",
            &[reference],
        );
        expect_exception(&mut ctx, "IllegalStateException");
        let id = location(ctx.rt, "test:acid");
        let missing = invoke(
            &mut ctx,
            REGISTRY,
            "getHolder",
            "(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/Optional;",
            &[reg, id],
        );
        assert_eq!(optional(&ctx, missing), None);
        assert_eq!(register(&mut ctx, reg, id, value), reference);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            invoke(&mut ctx, REFERENCE, "isBound", "()Z", &[reference]),
            V::TRUE
        );
        let found = invoke(
            &mut ctx,
            REGISTRY,
            "getHolder",
            "(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/Optional;",
            &[reg, id],
        );
        assert_eq!(optional(&ctx, found), Some(reference));
        let key = invoke(
            &mut ctx,
            REFERENCE,
            "key",
            "()Lnet/minecraft/resources/ResourceKey;",
            &[reference],
        );
        assert_eq!(
            crate::minecraft::resource_key_parts(ctx.rt, key),
            Some(("test:chemical".into(), "test:acid".into()))
        );
        let codec = invoke(
            &mut ctx,
            REGISTRY,
            "holderByNameCodec",
            "()Lcom/mojang/serialization/Codec;",
            &[reg],
        );
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let encoded = invoke(&mut ctx, "com/mojang/serialization/Codec", "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, reference]);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "{:?}",
            ctx.rt.pending_exception
        );
        let json = invoke(
            &mut ctx,
            "com/mojang/serialization/DataResult",
            "getOrThrow",
            "()Ljava/lang/Object;",
            &[encoded],
        );
        let decoded = invoke(&mut ctx, "com/mojang/serialization/Codec", "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, json]);
        assert_eq!(
            invoke(
                &mut ctx,
                "com/mojang/serialization/DataResult",
                "getOrThrow",
                "()Ljava/lang/Object;",
                &[decoded]
            ),
            reference
        );
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn standalone_refs_share_registration_identity_and_aliases_without_creating_missing_holders() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ReferenceHost(RegistryStore::new());
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let reg = registry(&mut ctx, "test:types", false);
        let value = ctx.rt.new_instance("test/Type", 0);
        let id = location(ctx.rt, "test:canonical");
        let reference = register(&mut ctx, reg, id, value);
        assert!(ctx.rt.pending_exception.is_none());
        let from = location(ctx.rt, "test:old");
        invoke(&mut ctx, REGISTRY, "addAlias", "(Lnet/minecraft/resources/ResourceLocation;Lnet/minecraft/resources/ResourceLocation;)V", &[reg, from, id]);
        assert!(ctx.rt.pending_exception.is_none());
        for key in [id, from] {
            let found = invoke(
                &mut ctx,
                REGISTRY,
                "getHolder",
                "(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/Optional;",
                &[reg, key],
            );
            assert_eq!(optional(&ctx, found), Some(reference));
        }
        let absent = location(ctx.rt, "test:missing");
        let missing = invoke(
            &mut ctx,
            REGISTRY,
            "getHolder",
            "(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/Optional;",
            &[reg, absent],
        );
        assert_eq!(optional(&ctx, missing), None);
        invoke(
            &mut ctx,
            REGISTRY,
            "createIntrusiveHolder",
            "(Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;",
            &[reg, value],
        );
        expect_exception(&mut ctx, "can't create intrusive");
    }
    #[test]
    fn duplicate_values_foreign_keys_and_registration_without_intrusive_preref_fail_atomically() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ReferenceHost(RegistryStore::new());
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let reg = registry(&mut ctx, "test:chemical", true);
        let value = ctx.rt.new_instance("test/Chemical", 0);
        let id = location(ctx.rt, "test:first");
        register(&mut ctx, reg, id, value);
        expect_exception(&mut ctx, "AssertionError");
        let reference = invoke(
            &mut ctx,
            REGISTRY,
            "createIntrusiveHolder",
            "(Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;",
            &[reg, value],
        );
        assert_eq!(register(&mut ctx, reg, id, value), reference);
        let other = ctx.rt.new_instance("test/Chemical", 0);
        invoke(
            &mut ctx,
            REGISTRY,
            "createIntrusiveHolder",
            "(Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;",
            &[reg, other],
        );
        register(&mut ctx, reg, id, other);
        expect_exception(&mut ctx, "duplicate key");
        let other_id = location(ctx.rt, "test:second");
        register(&mut ctx, reg, other_id, value);
        expect_exception(&mut ctx, "duplicate value");
        let foreign_key = crate::minecraft::new_resource_key(ctx.rt, "test:other", "test:second");
        invoke_member(&mut ctx, op::INVOKESTATIC, &MemberRef::new(REGISTRY, "register", "(Lnet/minecraft/core/Registry;Lnet/minecraft/resources/ResourceKey;Ljava/lang/Object;)Ljava/lang/Object;"), &[reg, foreign_key, other]);
        expect_exception(&mut ctx, "IllegalArgumentException");
        let bad_id = location(ctx.rt, "test:UPPERCASE");
        register(&mut ctx, reg, bad_id, other);
        expect_exception(&mut ctx, "IllegalArgumentException");
        let found = invoke(
            &mut ctx,
            REGISTRY,
            "getHolder",
            "(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/Optional;",
            &[reg, id],
        );
        assert_eq!(optional(&ctx, found), Some(reference));
        assert_eq!(
            invoke(
                &mut ctx,
                REFERENCE,
                "value",
                "()Ljava/lang/Object;",
                &[reference]
            ),
            value
        );
        let foreign = registry(&mut ctx, "test:foreign", false);
        let owner = invoke(
            &mut ctx,
            REGISTRY,
            "holderOwner",
            "()Lnet/minecraft/core/HolderOwner;",
            &[foreign],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                REFERENCE,
                "canSerializeIn",
                "(Lnet/minecraft/core/HolderOwner;)Z",
                &[reference, owner]
            ),
            V::FALSE
        );
    }
    #[test]
    fn freeze_checks_unbound_intrusive_refs_and_registry_flags_without_discarding_objects() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ReferenceHost(RegistryStore::new());
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let reg = registry(&mut ctx, "test:freeze", true);
        let value = ctx.rt.new_instance("test/Chemical", 0);
        let reference = invoke(
            &mut ctx,
            REGISTRY,
            "createIntrusiveHolder",
            "(Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;",
            &[reg, value],
        );
        invoke(
            &mut ctx,
            REGISTRY,
            "freeze",
            "()Lnet/minecraft/core/Registry;",
            &[reg],
        );
        expect_exception(&mut ctx, "not registered");
        assert_eq!(
            invoke(
                &mut ctx,
                REFERENCE,
                "value",
                "()Ljava/lang/Object;",
                &[reference]
            ),
            value
        );
        invoke(
            &mut ctx,
            REGISTRY,
            "createIntrusiveHolder",
            "(Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;",
            &[reg, value],
        );
        expect_exception(&mut ctx, "frozen");
        let id = location(ctx.rt, "test:late");
        register(&mut ctx, reg, id, value);
        expect_exception(&mut ctx, "frozen");
    }
}
