//! NeoForge Subsystems implemented in Rust:
//! - `ModConfigSpec` (`ModConfigSpec$Builder`, `BooleanValue`, `IntValue`, `DoubleValue`, `ConfigValue<T>`, TOML loading/saving)
//! - Capabilities (`BlockCapability`, `EntityCapability`, `ItemCapability`, `RegisterCapabilitiesEvent`)
//! - Data Attachments (`AttachmentType`, `IAttachmentHolder.getData` / `setData` / `hasData` / `removeData`)
//! - Custom Payload Networking (`RegisterPayloadHandlersEvent`, `PayloadRegistrar`, `PacketDistributor`)
//! - Brigadier Command Tree (`Commands.literal`, `Commands.argument`, `CommandDispatcher`, execution)

use mmga_runtime::{Rt, V};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// =========================================================================
// 1. NeoForge `ModConfigSpec` & TOML Persistence
// =========================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConfigPrimitive {
    Bool(bool),
    Int(i32),
    Long(i64),
    Double(f64),
    Text(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEntrySpec {
    pub entry_id: u64,
    pub path: Vec<String>,
    pub comment: Option<String>,
    pub default_val: ConfigPrimitive,
    pub current_val: ConfigPrimitive,
    pub min_num: Option<f64>,
    pub max_num: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigBuilderState {
    pub prefix_stack: Vec<String>,
    pub pending_comment: Option<String>,
    pub entries: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltConfigSpec {
    pub spec_id: u64,
    pub mod_id: Option<String>,
    pub config_type: String,
    pub file_name: Option<String>,
    pub entry_ids: Vec<u64>,
}

#[derive(Debug, Default, Clone)]
pub struct ConfigSubsystem {
    next_id: u64,
    pub builders: BTreeMap<u64, ConfigBuilderState>,
    pub specs: BTreeMap<u64, BuiltConfigSpec>,
    pub values: BTreeMap<u64, ConfigEntrySpec>,
}

impl ConfigSubsystem {
    pub fn new_builder(&mut self) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.builders.insert(id, ConfigBuilderState::default());
        id
    }

    pub fn push_section(&mut self, builder_id: u64, section: &str) {
        if let Some(b) = self.builders.get_mut(&builder_id) {
            for seg in section.split('.').filter(|s| !s.is_empty()) {
                b.prefix_stack.push(seg.to_owned());
            }
        }
    }

    pub fn pop_section(&mut self, builder_id: u64, count: usize) {
        if let Some(b) = self.builders.get_mut(&builder_id) {
            for _ in 0..count {
                let _ = b.prefix_stack.pop();
            }
        }
    }

    pub fn set_comment(&mut self, builder_id: u64, comment: String) {
        if let Some(b) = self.builders.get_mut(&builder_id) {
            b.pending_comment = Some(comment);
        }
    }

    pub fn define_entry(
        &mut self,
        builder_id: u64,
        key: &str,
        default_val: ConfigPrimitive,
        bounds: Option<(f64, f64)>,
    ) -> u64 {
        self.next_id += 1;
        let entry_id = self.next_id;
        let (mut path, comment) = self
            .builders
            .get_mut(&builder_id)
            .map(|b| {
                let mut p = b.prefix_stack.clone();
                for seg in key.split('.').filter(|s| !s.is_empty()) {
                    p.push(seg.to_owned());
                }
                b.entries.push(entry_id);
                (p, b.pending_comment.take())
            })
            .unwrap_or_else(|| (vec![key.to_owned()], None));
        if path.is_empty() {
            path.push(key.to_owned());
        }
        self.values.insert(
            entry_id,
            ConfigEntrySpec {
                entry_id,
                path,
                comment,
                current_val: default_val.clone(),
                default_val,
                min_num: bounds.map(|b| b.0),
                max_num: bounds.map(|b| b.1),
            },
        );
        entry_id
    }

    pub fn build_spec(&mut self, builder_id: u64) -> u64 {
        self.next_id += 1;
        let spec_id = self.next_id;
        let entry_ids = self
            .builders
            .get(&builder_id)
            .map(|b| b.entries.clone())
            .unwrap_or_default();
        self.specs.insert(
            spec_id,
            BuiltConfigSpec {
                spec_id,
                mod_id: None,
                config_type: "COMMON".into(),
                file_name: None,
                entry_ids,
            },
        );
        spec_id
    }

    /// Attaches a built spec to a mod file (`ModConfig$Type`: `COMMON`, `CLIENT`, `SERVER`, `STARTUP`).
    pub fn assign_mod(
        &mut self,
        spec_id: u64,
        mod_id: &str,
        config_type: &str,
        file_name: Option<String>,
    ) {
        if let Some(spec) = self.specs.get_mut(&spec_id) {
            spec.mod_id = Some(mod_id.to_owned());
            spec.config_type = config_type.to_owned();
            spec.file_name = file_name;
        }
    }

    pub fn current(&self, entry_id: u64) -> Option<&ConfigPrimitive> {
        self.values.get(&entry_id).map(|entry| &entry.current_val)
    }

    /// Loads overrides from a TOML string into `spec_id`'s config entries, clamping numeric ranges!
    pub fn apply_toml_overrides(&mut self, spec_id: u64, toml_text: &str) -> Result<usize, String> {
        let doc: toml::Value =
            toml::from_str(toml_text).map_err(|e| format!("invalid config TOML: {e}"))?;
        let Some(spec) = self.specs.get(&spec_id) else {
            return Ok(0);
        };
        let ids = spec.entry_ids.clone();
        let mut updated = 0usize;
        for eid in ids {
            let Some(entry) = self.values.get_mut(&eid) else {
                continue;
            };
            let mut cursor = Some(&doc);
            for seg in &entry.path {
                cursor = cursor.and_then(|v| v.as_table()).and_then(|t| t.get(seg));
            }
            let Some(val) = cursor else {
                continue;
            };
            match (&entry.default_val, val) {
                (ConfigPrimitive::Bool(_), toml::Value::Boolean(b)) => {
                    entry.current_val = ConfigPrimitive::Bool(*b);
                    updated += 1;
                }
                (ConfigPrimitive::Int(_), toml::Value::Integer(i)) => {
                    let mut n = *i as f64;
                    if let (Some(lo), Some(hi)) = (entry.min_num, entry.max_num) {
                        n = n.clamp(lo, hi);
                    }
                    entry.current_val = ConfigPrimitive::Int(n as i32);
                    updated += 1;
                }
                (ConfigPrimitive::Long(_), toml::Value::Integer(i)) => {
                    let mut n = *i as f64;
                    if let (Some(lo), Some(hi)) = (entry.min_num, entry.max_num) {
                        n = n.clamp(lo, hi);
                    }
                    entry.current_val = ConfigPrimitive::Long(n as i64);
                    updated += 1;
                }
                (ConfigPrimitive::Double(_), toml::Value::Float(f)) => {
                    let mut n = *f;
                    if let (Some(lo), Some(hi)) = (entry.min_num, entry.max_num) {
                        n = n.clamp(lo, hi);
                    }
                    entry.current_val = ConfigPrimitive::Double(n);
                    updated += 1;
                }
                (ConfigPrimitive::Text(_), toml::Value::String(s)) => {
                    entry.current_val = ConfigPrimitive::Text(s.clone());
                    updated += 1;
                }
                _ => {}
            }
        }
        Ok(updated)
    }

    /// Exports `spec_id`'s configuration entries as canonical NeoForge TOML text.
    pub fn render_toml(&self, spec_id: u64) -> String {
        let Some(spec) = self.specs.get(&spec_id) else {
            return String::new();
        };
        let mut lines = Vec::new();
        for &eid in &spec.entry_ids {
            let Some(e) = self.values.get(&eid) else {
                continue;
            };
            if let Some(c) = &e.comment {
                for cline in c.lines() {
                    lines.push(format!("# {cline}"));
                }
            }
            let dotted = e.path.join(".");
            let val_str = match &e.current_val {
                ConfigPrimitive::Bool(b) => b.to_string(),
                ConfigPrimitive::Int(i) => i.to_string(),
                ConfigPrimitive::Long(l) => l.to_string(),
                ConfigPrimitive::Double(d) => format!("{d:?}"),
                ConfigPrimitive::Text(s) => format!("{s:?}"),
            };
            lines.push(format!("{dotted} = {val_str}"));
        }
        lines.join("\n")
    }
}

// =========================================================================
// 2. NeoForge Capabilities (`BlockCapability`, `EntityCapability`, `ItemCapability`)
// =========================================================================

#[derive(Debug, Clone)]
pub struct CapabilityRegistration {
    pub capability_id: String,
    pub target_kind: &'static str, // "block", "block_entity", "entity", "item"
    pub target_ids: Vec<String>,
    pub provider_closure: V,
}

#[derive(Debug, Default, Clone)]
pub struct CapabilityStore {
    pub registrations: Vec<CapabilityRegistration>,
}

impl CapabilityStore {
    pub fn register(
        &mut self,
        rt: &mut Rt,
        capability_id: String,
        target_kind: &'static str,
        target_ids: Vec<String>,
        provider_closure: V,
    ) {
        rt.pin(provider_closure);
        self.registrations.push(CapabilityRegistration {
            capability_id,
            target_kind,
            target_ids,
            provider_closure,
        });
    }

    pub fn find_provider(
        &self,
        capability_id: &str,
        target_kind: &str,
        target_id: &str,
    ) -> Option<V> {
        self.registrations
            .iter()
            .find(|r| {
                r.capability_id == capability_id
                    && r.target_kind == target_kind
                    && (r.target_ids.is_empty() || r.target_ids.iter().any(|t| t == target_id))
            })
            .map(|r| r.provider_closure)
    }
}

// =========================================================================
// 3. NeoForge Data Attachments (`AttachmentType<T>` & `IAttachmentHolder`)
// =========================================================================

#[derive(Debug, Clone)]
pub struct AttachmentTypeDef {
    pub type_id: u64,
    pub default_supplier: V,
    pub copy_on_death: bool,
}

#[derive(Debug, Default, Clone)]
pub struct AttachmentStore {
    next_id: u64,
    pub types: BTreeMap<u64, AttachmentTypeDef>,
    /// `(holder_key, attachment_type_id) -> V`
    pub attached_data: BTreeMap<(String, u64), V>,
}

impl AttachmentStore {
    pub fn define_type(&mut self, rt: &mut Rt, default_supplier: V, copy_on_death: bool) -> u64 {
        rt.pin(default_supplier);
        self.next_id += 1;
        let id = self.next_id;
        self.types.insert(
            id,
            AttachmentTypeDef {
                type_id: id,
                default_supplier,
                copy_on_death,
            },
        );
        id
    }

    pub fn set_data(&mut self, rt: &mut Rt, holder_key: &str, type_id: u64, value: V) {
        rt.pin(value);
        self.attached_data
            .insert((holder_key.to_owned(), type_id), value);
    }

    pub fn has_data(&self, holder_key: &str, type_id: u64) -> bool {
        self.attached_data
            .contains_key(&(holder_key.to_owned(), type_id))
    }

    pub fn get_data(&self, holder_key: &str, type_id: u64) -> Option<V> {
        self.attached_data
            .get(&(holder_key.to_owned(), type_id))
            .copied()
    }

    /// Returns the previous value. Missing data yields `None` (NeoForge `removeData`).
    pub fn remove_data(&mut self, holder_key: &str, type_id: u64) -> Option<V> {
        self.attached_data.remove(&(holder_key.to_owned(), type_id))
    }
}

// =========================================================================
// 4. Custom Payload Networking (`RegisterPayloadHandlersEvent`)
// =========================================================================

#[derive(Debug, Clone)]
pub struct RegisteredPayloadChannel {
    pub mod_namespace: String,
    pub protocol_version: String,
    pub payload_id: String,
    pub direction: &'static str, // "play_to_server", "play_to_client", "bidirectional"
    pub handler_closure: V,
}

#[derive(Debug, Default, Clone)]
pub struct NetworkStore {
    pub channels: Vec<RegisteredPayloadChannel>,
    pub sent_packets: Vec<(String, Vec<u8>)>,
}

impl NetworkStore {
    pub fn register_channel(
        &mut self,
        rt: &mut Rt,
        mod_namespace: impl Into<String>,
        protocol_version: impl Into<String>,
        payload_id: impl Into<String>,
        direction: &'static str,
        handler_closure: V,
    ) {
        rt.pin(handler_closure);
        self.channels.push(RegisteredPayloadChannel {
            mod_namespace: mod_namespace.into(),
            protocol_version: protocol_version.into(),
            payload_id: payload_id.into(),
            direction,
            handler_closure,
        });
    }

    pub fn send(&mut self, payload_id: impl Into<String>, bytes: Vec<u8>) {
        self.sent_packets.push((payload_id.into(), bytes));
    }
}

// =========================================================================
// 5. Brigadier Command Dispatcher (`Commands.literal`, `Commands.argument`)
// =========================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArgumentKind {
    Word,
    GreedyString,
    Integer,
    Float,
    Bool,
    Opaque,
}

#[derive(Debug, Clone)]
pub enum CommandNodeKind {
    Literal(String),
    Argument { name: String, kind: ArgumentKind },
}

#[derive(Debug, Clone)]
pub struct BrigadierNode {
    pub node_id: u64,
    pub kind: CommandNodeKind,
    pub executes: Option<V>,
    pub argument_type: Option<V>,
    pub builder_callbacks: BTreeMap<String, V>,
    pub requirement_is_default: bool,
    pub children: Vec<u64>,
}

#[derive(Debug, Default, Clone)]
pub struct BrigadierTree {
    next_id: u64,
    default_requirement: Option<V>,
    pub nodes: BTreeMap<u64, BrigadierNode>,
    pub roots: Vec<u64>,
}

impl BrigadierTree {
    pub fn alloc_node(&mut self, kind: CommandNodeKind) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.nodes.insert(
            id,
            BrigadierNode {
                node_id: id,
                kind,
                executes: None,
                argument_type: None,
                builder_callbacks: BTreeMap::new(),
                requirement_is_default: true,
                children: Vec::new(),
            },
        );
        id
    }

    pub fn set_executes(&mut self, rt: &mut Rt, node_id: u64, cmd_closure: V) {
        rt.pin(cmd_closure);
        if let Some(n) = self.nodes.get_mut(&node_id) {
            n.executes = (!cmd_closure.is_null()).then_some(cmd_closure);
        }
    }
    pub fn set_argument_type(&mut self, rt: &mut Rt, node_id: u64, argument: V) {
        rt.pin(argument);
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.argument_type = Some(argument);
        }
    }
    pub fn set_builder_callback(&mut self, rt: &mut Rt, node_id: u64, name: &str, value: V) {
        rt.pin(value);
        if let Some(node) = self.nodes.get_mut(&node_id) {
            if name == "requires" {
                node.requirement_is_default = self.default_requirement == Some(value);
            }
            node.builder_callbacks.insert(name.into(), value);
        }
    }

    /// Brigadier 1.3.10 ArgumentBuilder's default `source -> true` predicate.
    /// A real closure keeps it callable through Predicate.test, including null.
    pub fn default_requirement(&mut self, rt: &mut Rt) -> V {
        if let Some(predicate) = self.default_requirement {
            return predicate;
        }
        let predicate = rt.alloc("java/util/function/Predicate", 0, mmga_runtime::Data::Closure(Box::new(mmga_runtime::ClosureData {
            interface_class: "java/util/function/Predicate".into(),
            sam_name: "test".into(),
            sam_desc: "(Ljava/lang/Object;)Z".into(),
            instantiated_desc: "(Ljava/lang/Object;)Z".into(),
            captures: vec![],
            target: mmga_runtime::ClosureTarget::MethodHandle {
                handle_kind: 6,
                member: mmga_classfile::MemberRef::new("mmga/internal/BrigadierDefaultRequirement", "test", "(Ljava/lang/Object;)Z"),
            },
        })));
        rt.pin(predicate);
        self.default_requirement = Some(predicate);
        predicate
    }

    pub fn requirement(&mut self, rt: &mut Rt, node_id: u64) -> Option<V> {
        let explicit = self.nodes.get(&node_id)?.builder_callbacks.get("requires").copied();
        Some(explicit.unwrap_or_else(|| self.default_requirement(rt)))
    }

    /// build() copies builder state, while its already-built children stay shared.
    pub fn snapshot_node(&mut self, node_id: u64) -> Option<u64> {
        let mut snapshot = self.nodes.get(&node_id)?.clone();
        self.next_id += 1;
        let id = self.next_id;
        snapshot.node_id = id;
        self.nodes.insert(id, snapshot);
        Some(id)
    }

    pub fn add_child(&mut self, parent_id: u64, child_id: u64) {
        if let Some(p) = self.nodes.get_mut(&parent_id) {
            if !p.children.contains(&child_id) {
                p.children.push(child_id);
            }
        }
    }

    pub fn register_root(&mut self, root_id: u64) {
        if !self.roots.contains(&root_id) {
            self.roots.push(root_id);
        }
    }
    pub fn as_vm_root_values(&self) -> Vec<V> {
        let mut roots = Vec::new();
        for node in self.nodes.values() {
            roots.push(node.executes.unwrap_or(V::NULL));
            roots.push(node.argument_type.unwrap_or(V::NULL));
            roots.extend(node.builder_callbacks.values().copied());
        }
        roots
    }

    /// Matches a slash command line (e.g. `"/kinetic speed 64"`) against registered Brigadier roots,
    /// returning `(matched_closure, parsed_arguments_map)`.
    pub fn match_command(
        &self,
        command_line: &str,
    ) -> Option<(V, BTreeMap<String, ConfigPrimitive>)> {
        self.match_command_checked(command_line).ok().flatten()
    }

    pub fn match_command_checked(
        &self,
        command_line: &str,
    ) -> Result<Option<(V, BTreeMap<String, ConfigPrimitive>)>, String> {
        let trimmed = command_line.trim().trim_start_matches('/');
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.is_empty() {
            return Ok(None);
        }
        let mut unsupported = None;
        for &root_id in &self.roots {
            let mut args = BTreeMap::new();
            match self.match_from(root_id, &tokens, 0, &mut args) {
                Ok(Some(exec)) => return Ok(Some((exec, args))),
                Ok(None) => {}
                Err(error) => unsupported = Some(error),
            }
        }
        if let Some(error) = unsupported {
            Err(error)
        } else {
            Ok(None)
        }
    }

    fn match_from(
        &self,
        node_id: u64,
        tokens: &[&str],
        idx: usize,
        args: &mut BTreeMap<String, ConfigPrimitive>,
    ) -> Result<Option<V>, String> {
        let Some(node) = self.nodes.get(&node_id) else {
            return Ok(None);
        };
        let Some(token) = tokens.get(idx).copied() else {
            return Ok(None);
        };
        if matches!(&node.kind,CommandNodeKind::Literal(literal) if literal!=token) {
            return Ok(None);
        }
        if !node.requirement_is_default {
            return Err(
                "Command node requires an unimplemented sender permission predicate".into(),
            );
        }
        if node.builder_callbacks.get("redirect").is_some_and(|value| !value.is_null())
            || node.builder_callbacks.get("fork").is_some_and(|value| value.i() != 0)
        {
            return Err("Command redirect/fork execution is not implemented".into());
        }
        let next_idx = match &node.kind {
            CommandNodeKind::Literal(lit) => {
                if lit != token {
                    return Ok(None);
                }
                idx + 1
            }
            CommandNodeKind::Argument { name, kind } => match kind {
                ArgumentKind::Integer => {
                    let Ok(v) = token.parse::<i32>() else {
                        return Ok(None);
                    };
                    args.insert(name.clone(), ConfigPrimitive::Int(v));
                    idx + 1
                }
                ArgumentKind::Float => {
                    let Ok(v) = token.parse::<f64>() else {
                        return Ok(None);
                    };
                    args.insert(name.clone(), ConfigPrimitive::Double(v));
                    idx + 1
                }
                ArgumentKind::Bool => {
                    let Ok(v) = token.parse::<bool>() else {
                        return Ok(None);
                    };
                    args.insert(name.clone(), ConfigPrimitive::Bool(v));
                    idx + 1
                }
                ArgumentKind::Word => {
                    args.insert(name.clone(), ConfigPrimitive::Text(token.to_owned()));
                    idx + 1
                }
                ArgumentKind::GreedyString => {
                    args.insert(name.clone(), ConfigPrimitive::Text(tokens[idx..].join(" ")));
                    tokens.len()
                }
                ArgumentKind::Opaque => {
                    return Err(format!(
                        "Argument `{name}` uses an unimplemented command parser"
                    ))
                }
            },
        };

        if next_idx == tokens.len() {
            return Ok(node.executes);
        }
        let mut unsupported = None;
        for &child_id in &node.children {
            let mut branch_args = args.clone();
            match self.match_from(child_id, tokens, next_idx, &mut branch_args) {
                Ok(Some(found)) => {
                    *args = branch_args;
                    return Ok(Some(found));
                }
                Ok(None) => {}
                Err(error) => unsupported = Some(error),
            }
        }
        if let Some(error) = unsupported {
            Err(error)
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::Rt;

    #[test]
    fn custom_brigadier_argument_is_retained_and_is_never_parsed_as_a_word() {
        let mut rt = Rt::new();
        let mut tree = BrigadierTree::default();
        let name = rt.intern_str("value");
        let argument = rt.new_instance("test/ActualArgumentType", 0);
        let builder=crate::minecraft::call_brigadier(&mut rt,&mut tree,&BTreeMap::new(),&mmga_classfile::MemberRef::new("net/minecraft/commands/Commands","argument","(Ljava/lang/String;Lcom/mojang/brigadier/arguments/ArgumentType;)Lcom/mojang/brigadier/builder/RequiredArgumentBuilder;"),true,&[name,argument]).unwrap();
        let id = rt.handle_of(builder).unwrap().1;
        assert!(
            !matches!(
                &tree.nodes[&id].kind,
                CommandNodeKind::Argument {
                    kind: ArgumentKind::Word,
                    ..
                }
            ),
            "Unknown argument type cannot become the word parser"
        );
        let got = crate::minecraft::call_brigadier(
            &mut rt,
            &mut tree,
            &BTreeMap::new(),
            &mmga_classfile::MemberRef::new(
                "com/mojang/brigadier/builder/RequiredArgumentBuilder",
                "getType",
                "()Lcom/mojang/brigadier/arguments/ArgumentType;",
            ),
            false,
            &[builder],
        );
        assert_eq!(got, Some(argument));
        let root = tree.alloc_node(CommandNodeKind::Literal("inspect".into()));
        tree.add_child(root, id);
        tree.register_root(root);
        let command = rt.new_instance("java/util/function/Function", 0);
        tree.set_executes(&mut rt, id, command);
        assert!(tree.match_command("/inspect arbitrary").is_none());
        let error = tree
            .match_command_checked("/inspect arbitrary")
            .unwrap_err();
        assert!(error.contains("unimplemented command parser"));
        let literal = tree.alloc_node(CommandNodeKind::Literal("known".into()));
        tree.add_child(root, literal);
        tree.set_executes(&mut rt, literal, command);
        assert_eq!(
            tree.match_command_checked("/inspect known")
                .unwrap()
                .unwrap()
                .0,
            command,
            "A real literal alternative remains available"
        );
        crate::minecraft::call_brigadier(&mut rt,&mut tree,&BTreeMap::new(),&mmga_classfile::MemberRef::new("net/minecraft/commands/Commands","argument","(Ljava/lang/String;Lcom/mojang/brigadier/arguments/ArgumentType;)Lcom/mojang/brigadier/builder/RequiredArgumentBuilder;"),true,&[name,V::NULL]);
        let error = rt.take_exception().unwrap();
        assert_eq!(rt.class_of(error), Some("java/lang/NullPointerException"));
    }

    #[test]
    fn builder_callbacks_are_preserved_and_permission_controls_do_not_become_unconditional() {
        let mut rt = Rt::new();
        let mut tree = BrigadierTree::default();
        let name = rt.intern_str("guarded");
        let predicate = rt.new_instance("java/util/function/Predicate", 0);
        let suggestions = rt.new_instance("com/mojang/brigadier/suggestion/SuggestionProvider", 0);
        let literal = crate::minecraft::call_brigadier(
            &mut rt,
            &mut tree,
            &BTreeMap::new(),
            &mmga_classfile::MemberRef::new(
                "net/minecraft/commands/Commands",
                "literal",
                "(Ljava/lang/String;)Lcom/mojang/brigadier/builder/LiteralArgumentBuilder;",
            ),
            true,
            &[name],
        )
        .unwrap();
        let id = rt.handle_of(literal).unwrap().1;
        crate::minecraft::call_brigadier(
            &mut rt,
            &mut tree,
            &BTreeMap::new(),
            &mmga_classfile::MemberRef::new(
                "com/mojang/brigadier/builder/LiteralArgumentBuilder",
                "requires",
                "(Ljava/util/function/Predicate;)Lcom/mojang/brigadier/builder/ArgumentBuilder;",
            ),
            false,
            &[literal, predicate],
        );
        crate::minecraft::call_brigadier(&mut rt,&mut tree,&BTreeMap::new(),&mmga_classfile::MemberRef::new("com/mojang/brigadier/builder/LiteralArgumentBuilder","suggests","(Lcom/mojang/brigadier/suggestion/SuggestionProvider;)Lcom/mojang/brigadier/builder/RequiredArgumentBuilder;"),false,&[literal,suggestions]);
        assert_eq!(
            tree.nodes[&id].builder_callbacks.get("requires"),
            Some(&predicate)
        );
        assert_eq!(
            tree.nodes[&id].builder_callbacks.get("suggests"),
            Some(&suggestions)
        );
        tree.register_root(id);
        let command = rt.new_instance("java/util/function/Function", 0);
        tree.set_executes(&mut rt, id, command);
        assert!(tree
            .match_command_checked("/guarded")
            .unwrap_err()
            .contains("permission predicate"));
        crate::minecraft::call_brigadier(
            &mut rt,
            &mut tree,
            &BTreeMap::new(),
            &mmga_classfile::MemberRef::new(
                "com/mojang/brigadier/builder/LiteralArgumentBuilder",
                "requires",
                "(Ljava/util/function/Predicate;)Lcom/mojang/brigadier/builder/ArgumentBuilder;",
            ),
            false,
            &[literal, V::NULL],
        );
        assert!(rt.pending_exception.is_none());
        assert_eq!(
            tree.nodes[&id].builder_callbacks.get("requires"),
            Some(&V::NULL)
        );
        assert!(tree.match_command_checked("/guarded").unwrap_err().contains("permission predicate"));
    }

    struct CommandTestHost(BrigadierTree);
    impl mmga_runtime::HostApi for CommandTestHost {
        fn get_static_field(&mut self, _: &mut Rt, _: &str, _: &str, _: &str) -> Option<V> { None }
        fn call_method(&mut self, rt: &mut Rt, _: &mmga_runtime::LinkedSpace, member: &mmga_classfile::MemberRef, is_static: bool, args: &[V]) -> Option<V> {
            crate::minecraft::call_brigadier(rt, &mut self.0, &BTreeMap::new(), member, is_static, args)
        }
    }
    const LB: &str = "com/mojang/brigadier/builder/LiteralArgumentBuilder";
    const LN: &str = "com/mojang/brigadier/tree/LiteralCommandNode";
    fn cmd(ctx: &mut mmga_runtime::ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        mmga_runtime::invoke_member(ctx, if name=="literal" {mmga_classfile::op::INVOKESTATIC}else{mmga_classfile::op::INVOKEVIRTUAL}, &mmga_classfile::MemberRef::new(owner,name,desc),args)
    }
    fn literal(ctx: &mut mmga_runtime::ExecCtx<'_>, name: &str) -> V {
        let text=ctx.rt.new_str(name);
        cmd(ctx,LB,"literal","(Ljava/lang/String;)Lcom/mojang/brigadier/builder/LiteralArgumentBuilder;",&[text])
    }
    fn build(ctx: &mut mmga_runtime::ExecCtx<'_>, value: V) -> V {
        cmd(ctx,LB,"build","()Lcom/mojang/brigadier/tree/LiteralCommandNode;",&[value])
    }
    #[test]
    fn catnip_build_redirect_copies_default_predicate_and_null_forward_state() {
        // Actual CatnipCommands.buildRedirect sequence; Brigadier 1.3.10 jar SHA1
        // d15b53a14cf20fdcaa98f731af5dda654452c010 verifies the default lambda.
        let mut rt=Rt::new(); let linked=mmga_runtime::LinkedSpace::new();
        let mut host=CommandTestHost(BrigadierTree::default());
        let (alias_id,command);
        {
            let mut ctx=mmga_runtime::ExecCtx{rt:&mut rt,linked:&linked,host:&mut host,depth:0};
            let original=literal(&mut ctx,"util"); let child=literal(&mut ctx,"flyspeed");
            command=ctx.rt.new_instance("test/Command",0);
            cmd(&mut ctx,LB,"executes","(Lcom/mojang/brigadier/Command;)Lcom/mojang/brigadier/builder/ArgumentBuilder;",&[child,command]);
            cmd(&mut ctx,LB,"then","(Lcom/mojang/brigadier/builder/ArgumentBuilder;)Lcom/mojang/brigadier/builder/ArgumentBuilder;",&[original,child]);
            let original=build(&mut ctx,original); let alias=literal(&mut ctx,"u");
            let requirement=cmd(&mut ctx,LN,"getRequirement","()Ljava/util/function/Predicate;",&[original]);
            assert!(!requirement.is_null());
            for source in [V::NULL,original] {
                assert_eq!(cmd(&mut ctx,"java/util/function/Predicate","test","(Ljava/lang/Object;)Z",&[requirement,source]),V::TRUE);
            }
            cmd(&mut ctx,LB,"requires","(Ljava/util/function/Predicate;)Lcom/mojang/brigadier/builder/ArgumentBuilder;",&[alias,requirement]);
            let target=cmd(&mut ctx,LN,"getRedirect","()Lcom/mojang/brigadier/tree/CommandNode;",&[original]);
            let modifier=cmd(&mut ctx,LN,"getRedirectModifier","()Lcom/mojang/brigadier/RedirectModifier;",&[original]);
            let fork=cmd(&mut ctx,LN,"isFork","()Z",&[original]);
            assert!(target.is_null()); assert!(modifier.is_null()); assert_eq!(fork,V::FALSE);
            cmd(&mut ctx,LB,"forward","(Lcom/mojang/brigadier/tree/CommandNode;Lcom/mojang/brigadier/RedirectModifier;Z)Lcom/mojang/brigadier/builder/ArgumentBuilder;",&[alias,target,modifier,fork]);
            let command_ref=cmd(&mut ctx,LN,"getCommand","()Lcom/mojang/brigadier/Command;",&[original]);
            cmd(&mut ctx,LB,"executes","(Lcom/mojang/brigadier/Command;)Lcom/mojang/brigadier/builder/ArgumentBuilder;",&[alias,command_ref]);
            let children=cmd(&mut ctx,LN,"getChildren","()Ljava/util/Collection;",&[original]);
            let children=match &ctx.rt.get_obj(children).unwrap().data { mmga_runtime::Data::List(xs)=>xs.clone(), _=>panic!("children") };
            for child in children { cmd(&mut ctx,LB,"then","(Lcom/mojang/brigadier/tree/CommandNode;)Lcom/mojang/brigadier/builder/ArgumentBuilder;",&[alias,child]); }
            let alias=build(&mut ctx,alias); alias_id=ctx.rt.handle_of(alias).unwrap().1;
            assert_eq!(cmd(&mut ctx,LN,"getRequirement","()Ljava/util/function/Predicate;",&[alias]),requirement);
            assert_eq!(cmd(&mut ctx,LN,"canUse","(Ljava/lang/Object;)Z",&[alias,V::NULL]),V::TRUE);
            assert!(ctx.rt.pending_exception.is_none());
        }
        host.0.register_root(alias_id);
        assert_eq!(host.0.match_command_checked("/u flyspeed").unwrap().unwrap().0,command);
    }

    #[test]
    fn brigadier_null_requirement_defers_npe_and_built_nodes_snapshot_state() {
        let mut rt=Rt::new(); let linked=mmga_runtime::LinkedSpace::new();
        let mut host=CommandTestHost(BrigadierTree::default());
        let mut ctx=mmga_runtime::ExecCtx{rt:&mut rt,linked:&linked,host:&mut host,depth:0};
        let builder=literal(&mut ctx,"guarded"); let command=ctx.rt.new_instance("test/Command",0);
        cmd(&mut ctx,LB,"executes","(Lcom/mojang/brigadier/Command;)Lcom/mojang/brigadier/builder/ArgumentBuilder;",&[builder,command]);
        let before=build(&mut ctx,builder);
        cmd(&mut ctx,LB,"executes","(Lcom/mojang/brigadier/Command;)Lcom/mojang/brigadier/builder/ArgumentBuilder;",&[builder,V::NULL]);
        cmd(&mut ctx,LB,"requires","(Ljava/util/function/Predicate;)Lcom/mojang/brigadier/builder/ArgumentBuilder;",&[builder,V::NULL]);
        let after=build(&mut ctx,builder);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(cmd(&mut ctx,LN,"getCommand","()Lcom/mojang/brigadier/Command;",&[before]),command);
        assert!(cmd(&mut ctx,LN,"getCommand","()Lcom/mojang/brigadier/Command;",&[after]).is_null());
        assert!(!cmd(&mut ctx,LN,"getRequirement","()Ljava/util/function/Predicate;",&[before]).is_null());
        assert!(cmd(&mut ctx,LN,"getRequirement","()Ljava/util/function/Predicate;",&[after]).is_null());
        cmd(&mut ctx,LN,"canUse","(Ljava/lang/Object;)Z",&[after,V::NULL]);
        let error=ctx.rt.take_exception().unwrap(); assert_eq!(ctx.rt.class_of(error),Some("java/lang/NullPointerException"));
        let redirected=literal(&mut ctx,"redirect"); let modifier=ctx.rt.new_instance("test/RedirectModifier",0);
        cmd(&mut ctx,LB,"forward","(Lcom/mojang/brigadier/tree/CommandNode;Lcom/mojang/brigadier/RedirectModifier;Z)Lcom/mojang/brigadier/builder/ArgumentBuilder;",&[redirected,before,modifier,V::TRUE]);
        let built=build(&mut ctx,redirected);
        assert_eq!(cmd(&mut ctx,LN,"getRedirect","()Lcom/mojang/brigadier/tree/CommandNode;",&[built]),before);
        assert_eq!(cmd(&mut ctx,LN,"getRedirectModifier","()Lcom/mojang/brigadier/RedirectModifier;",&[built]),modifier);
        assert_eq!(cmd(&mut ctx,LN,"isFork","()Z",&[built]),V::TRUE);
        cmd(&mut ctx,LB,"then","(Lcom/mojang/brigadier/tree/CommandNode;)Lcom/mojang/brigadier/builder/ArgumentBuilder;",&[redirected,before]);
        let error=ctx.rt.take_exception().unwrap(); assert_eq!(ctx.rt.class_of(error),Some("java/lang/IllegalStateException"));
        drop(ctx);
        let built_id=rt.handle_of(built).unwrap().1;
        host.0.register_root(built_id);
        assert!(host.0.match_command_checked("/redirect").unwrap_err().contains("redirect/fork"));
        let after_id=rt.handle_of(after).unwrap().1;
        host.0.register_root(after_id);
        assert!(host.0.match_command_checked("/guarded").unwrap_err().contains("permission predicate"));
    }

    #[test]
    fn config_spec_clamps_toml_overrides() {
        let mut cfg = ConfigSubsystem::default();
        let builder = cfg.new_builder();
        cfg.push_section(builder, "general");
        cfg.set_comment(builder, "Ticks between pulses".into());
        let speed = cfg.define_entry(builder, "speed", ConfigPrimitive::Int(4), Some((1.0, 20.0)));
        cfg.pop_section(builder, 1);
        let enabled = cfg.define_entry(builder, "enabled", ConfigPrimitive::Bool(true), None);
        let spec = cfg.build_spec(builder);
        cfg.assign_mod(
            spec,
            "examplemod",
            "COMMON",
            Some("examplemod-common.toml".into()),
        );

        let updated = cfg
            .apply_toml_overrides(spec, "general.speed = 100\nenabled = false\n")
            .unwrap();
        assert_eq!(updated, 2);
        assert_eq!(cfg.current(speed), Some(&ConfigPrimitive::Int(20)));
        assert_eq!(cfg.current(enabled), Some(&ConfigPrimitive::Bool(false)));
        let rendered = cfg.render_toml(spec);
        assert!(rendered.contains("# Ticks between pulses"));
        assert!(rendered.contains("general.speed = 20"));
        assert!(rendered.contains("enabled = false"));
        assert!(cfg.apply_toml_overrides(spec, "not toml {").is_err());
    }

    #[test]
    fn capabilities_attachments_network_and_commands() {
        let mut rt = Rt::new();
        let mut caps = CapabilityStore::default();
        let provider = rt.intern_str("energy");
        caps.register(
            &mut rt,
            "examplemod:energy".into(),
            "block",
            vec!["examplemod:example_block".into()],
            provider,
        );
        assert_eq!(
            caps.find_provider("examplemod:energy", "block", "examplemod:example_block"),
            Some(provider)
        );
        assert!(caps
            .find_provider("examplemod:energy", "block", "minecraft:stone")
            .is_none());

        let mut attachments = AttachmentStore::default();
        let supplier = rt.intern_str("default-mana");
        let type_id = attachments.define_type(&mut rt, supplier, true);
        assert!(attachments.types[&type_id].copy_on_death);
        assert!(!attachments.has_data("player:1", type_id));
        let mana = rt.intern_str("mana");
        attachments.set_data(&mut rt, "player:1", type_id, mana);
        assert_eq!(attachments.get_data("player:1", type_id), Some(mana));
        assert_eq!(attachments.remove_data("player:1", type_id), Some(mana));
        assert!(!attachments.has_data("player:1", type_id));

        let mut network = NetworkStore::default();
        let handler = rt.intern_str("handler");
        network.register_channel(
            &mut rt,
            "examplemod",
            "1",
            "examplemod:pulse",
            "play_to_server",
            handler,
        );
        network.send("examplemod:pulse", vec![1, 2, 3]);
        assert_eq!(network.channels[0].handler_closure, handler);
        assert_eq!(
            network.sent_packets,
            vec![("examplemod:pulse".into(), vec![1, 2, 3])]
        );

        let mut tree = BrigadierTree::default();
        let root = tree.alloc_node(CommandNodeKind::Literal("kinetic".into()));
        let speed = tree.alloc_node(CommandNodeKind::Literal("speed".into()));
        let amount = tree.alloc_node(CommandNodeKind::Argument {
            name: "amount".into(),
            kind: ArgumentKind::Integer,
        });
        let executes = rt.intern_str("exec");
        tree.set_executes(&mut rt, amount, executes);
        tree.add_child(speed, amount);
        tree.add_child(root, speed);
        tree.register_root(root);
        let (got, args) = tree.match_command("/kinetic speed 64").unwrap();
        assert_eq!(got, executes);
        assert_eq!(args.get("amount"), Some(&ConfigPrimitive::Int(64)));
        assert!(tree.match_command("/unknown").is_none());
    }
}
