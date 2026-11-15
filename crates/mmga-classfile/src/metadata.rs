//! NeoForge mod metadata (`META-INF/neoforge.mods.toml`, `META-INF/mods.toml`),
//! `META-INF/MANIFEST.MF`, Mixin configuration JSON (`*.mixins.json`),
//! Jar-in-Jar (`META-INF/jarjar/metadata.json`), and topological dependency ordering.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Language loader declared in `neoforge.mods.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModLoaderKind {
    JavaFml,
    LowCodeFml,
    KotlinForForge,
    Other(String),
}

impl ModLoaderKind {
    pub fn parse(raw: &str) -> Self {
        match raw.trim() {
            "" | "javafml" => Self::JavaFml,
            "lowcodefml" => Self::LowCodeFml,
            "kotlinforforge" => Self::KotlinForForge,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// Dependency requirement type in NeoForge 21.1+ / 26.3+ (`type = "required" | ...`) or legacy Forge (`mandatory = true`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Discouraged,
}

/// Load ordering constraint relative to a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyOrdering {
    None,
    Before,
    After,
}

/// Side restriction for a dependency or subscriber.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModSide {
    Both,
    Client,
    Server,
}

impl ModSide {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_uppercase().as_str() {
            "CLIENT" => Self::Client,
            "SERVER" | "DEDICATED_SERVER" => Self::Server,
            _ => Self::Both,
        }
    }

    pub fn matches_server(&self, is_dedicated_server: bool) -> bool {
        match self {
            Self::Both => true,
            Self::Client => !is_dedicated_server,
            Self::Server => is_dedicated_server,
        }
    }
}

/// One `[[dependencies.<modId>]]` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModDependency {
    pub mod_id: String,
    pub dep_type: DependencyType,
    pub version_range: String,
    pub ordering: DependencyOrdering,
    pub side: ModSide,
}

/// One `[[mixins]]` entry in `neoforge.mods.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixinDeclaration {
    pub config: String,
    pub behavior_version: Option<String>,
}

/// One `[[mods]]` entry in `neoforge.mods.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModEntry {
    pub mod_id: String,
    pub namespace: String,
    pub version: String,
    pub display_name: String,
    pub description: String,
    pub authors: String,
    pub enum_extensions: Option<String>,
}

/// Parsed `neoforge.mods.toml` / `mods.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModsToml {
    pub mod_loader: ModLoaderKind,
    pub loader_version: String,
    pub license: String,
    pub mods: Vec<ModEntry>,
    pub mixins: Vec<MixinDeclaration>,
    pub access_transformers: Vec<String>,
    pub dependencies: BTreeMap<String, Vec<ModDependency>>,
}

impl ModsToml {
    /// Parses `neoforge.mods.toml` or `mods.toml`, substituting `${file.jarVersion}` from `jar_version`.
    pub fn parse(toml_str: &str, jar_version: Option<&str>) -> Result<Self, String> {
        let doc: toml::Value = toml::from_str(toml_str)
            .map_err(|e| format!("failed to parse mods.toml: {e}"))?;
        let table = doc
            .as_table()
            .ok_or_else(|| "mods.toml root must be a table".to_owned())?;

        let get_str = |t: &toml::map::Map<String, toml::Value>, key: &str| -> String {
            t.get(key)
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_owned()
        };

        let mod_loader = ModLoaderKind::parse(&get_str(table, "modLoader"));
        let loader_version = get_str(table, "loaderVersion");
        let license = get_str(table, "license");

        let mut mods = Vec::new();
        if let Some(arr) = table.get("mods").and_then(toml::Value::as_array) {
            for item in arr {
                let Some(m) = item.as_table() else {
                    continue;
                };
                let mod_id = get_str(m, "modId");
                if mod_id.is_empty() {
                    continue;
                }
                let namespace = {
                    let ns = get_str(m, "namespace");
                    if ns.is_empty() {
                        mod_id.clone()
                    } else {
                        ns
                    }
                };
                let raw_ver = get_str(m, "version");
                let version = if raw_ver.is_empty() || raw_ver.contains("${file.jarVersion}") {
                    jar_version.unwrap_or("0.0.0-dev").to_owned()
                } else {
                    raw_ver
                };
                let display_name = {
                    let dn = get_str(m, "displayName");
                    if dn.is_empty() {
                        mod_id.clone()
                    } else {
                        dn
                    }
                };
                let description = get_str(m, "description");
                let authors = get_str(m, "authors");
                let enum_extensions = m
                    .get("enumExtensions")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned);
                mods.push(ModEntry {
                    mod_id,
                    namespace,
                    version,
                    display_name,
                    description,
                    authors,
                    enum_extensions,
                });
            }
        }

        let mut mixins = Vec::new();
        if let Some(arr) = table.get("mixins").and_then(toml::Value::as_array) {
            for item in arr {
                if let Some(t) = item.as_table() {
                    let cfg = get_str(t, "config");
                    if !cfg.is_empty() {
                        mixins.push(MixinDeclaration {
                            config: cfg,
                            behavior_version: t
                                .get("behaviorVersion")
                                .and_then(toml::Value::as_str)
                                .map(str::to_owned),
                        });
                    }
                }
            }
        }

        let mut access_transformers = Vec::new();
        if let Some(arr) = table
            .get("accessTransformers")
            .and_then(toml::Value::as_array)
        {
            for item in arr {
                if let Some(t) = item.as_table() {
                    let file = get_str(t, "file");
                    if !file.is_empty() {
                        access_transformers.push(file);
                    }
                } else if let Some(s) = item.as_str() {
                    access_transformers.push(s.to_owned());
                }
            }
        }

        let mut dependencies = BTreeMap::new();
        if let Some(dep_root) = table.get("dependencies").and_then(toml::Value::as_table) {
            for (owner_id, list_val) in dep_root {
                let Some(arr) = list_val.as_array() else {
                    continue;
                };
                let mut parsed_deps = Vec::new();
                for item in arr {
                    let Some(dt) = item.as_table() else {
                        continue;
                    };
                    let dep_mod_id = get_str(dt, "modId");
                    if dep_mod_id.is_empty() {
                        continue;
                    }
                    let dep_type = if let Some(ty_str) = dt.get("type").and_then(toml::Value::as_str)
                    {
                        match ty_str.trim().to_ascii_lowercase().as_str() {
                            "optional" => DependencyType::Optional,
                            "incompatible" => DependencyType::Incompatible,
                            "discouraged" => DependencyType::Discouraged,
                            _ => DependencyType::Required,
                        }
                    } else if let Some(mandatory) = dt.get("mandatory").and_then(toml::Value::as_bool)
                    {
                        if mandatory {
                            DependencyType::Required
                        } else {
                            DependencyType::Optional
                        }
                    } else {
                        DependencyType::Required
                    };

                    let ordering = match get_str(dt, "ordering").to_ascii_uppercase().as_str() {
                        "BEFORE" => DependencyOrdering::Before,
                        "AFTER" => DependencyOrdering::After,
                        _ => DependencyOrdering::None,
                    };
                    let side = ModSide::parse(&get_str(dt, "side"));
                    let version_range = get_str(dt, "versionRange");
                    parsed_deps.push(ModDependency {
                        mod_id: dep_mod_id,
                        dep_type,
                        version_range,
                        ordering,
                        side,
                    });
                }
                dependencies.insert(owner_id.clone(), parsed_deps);
            }
        }

        Ok(ModsToml {
            mod_loader,
            loader_version,
            license,
            mods,
            mixins,
            access_transformers,
            dependencies,
        })
    }
}

/// Parses `META-INF/MANIFEST.MF` into header key-value pairs (handling RFC 822 line continuations).
pub fn parse_manifest_mf(text: &str) -> BTreeMap<String, String> {
    let mut unfolded: Vec<String> = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim_end_matches('\r');
        if let Some(cont) = line.strip_prefix(' ') {
            if let Some(last) = unfolded.last_mut() {
                last.push_str(cont);
            }
        } else if !line.is_empty() {
            unfolded.push(line.to_owned());
        }
    }
    let mut map = BTreeMap::new();
    for line in unfolded {
        if let Some((k, v)) = line.split_once(':') {
            map.insert(k.trim().to_owned(), v.trim().to_owned());
        }
    }
    map
}

/// Parsed SpongePowered Mixin configuration JSON (`*.mixins.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixinConfig {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub package: String,
    #[serde(default, rename = "compatibilityLevel")]
    pub compatibility_level: Option<String>,
    #[serde(default)]
    pub mixins: Vec<String>,
    #[serde(default)]
    pub client: Vec<String>,
    #[serde(default)]
    pub server: Vec<String>,
    #[serde(default)]
    pub plugin: Option<String>,
}

impl MixinConfig {
    pub fn parse(json_str: &str) -> Result<Self, String> {
        serde_json::from_str(json_str).map_err(|e| format!("invalid mixin config JSON: {e}"))
    }

    /// Returns internal class names (`pkg/MixinName`) active on the requested side.
    pub fn active_mixin_classes(&self, is_dedicated_server: bool) -> Vec<String> {
        let prefix = self.package.trim_matches('.').replace('.', "/");
        let qualify = |rel: &str| -> String {
            let rel_slash = rel.trim_matches('.').replace('.', "/");
            if prefix.is_empty() {
                rel_slash
            } else {
                format!("{prefix}/{rel_slash}")
            }
        };
        let mut out: Vec<String> = self.mixins.iter().map(|m| qualify(m)).collect();
        if is_dedicated_server {
            out.extend(self.server.iter().map(|m| qualify(m)));
        } else {
            out.extend(self.client.iter().map(|m| qualify(m)));
        }
        out
    }
}

/// Nested Jar-in-Jar metadata (`META-INF/jarjar/metadata.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JarJarMetadata {
    #[serde(default)]
    pub jars: Vec<JarJarEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JarJarEntry {
    pub path: String,
    #[serde(default)]
    pub identifier: JarJarIdentifier,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JarJarIdentifier {
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub artifact: String,
}

/// Resolves mod load order across multiple `ModsToml` descriptors, checking required & incompatible
/// dependencies (treating `minecraft`, `neoforge`, `fml`, and `java` as built-in platform mods).
pub fn resolve_mod_load_order(
    manifests: &[&ModsToml],
    is_dedicated_server: bool,
) -> Result<Vec<String>, String> {
    let builtin: BTreeSet<&str> = ["minecraft", "neoforge", "forge", "fml", "java"]
        .into_iter()
        .collect();

    let mut all_mods = Vec::new();
    let mut known_set = BTreeSet::new();
    for m in manifests {
        for entry in &m.mods {
            if known_set.insert(entry.mod_id.clone()) {
                all_mods.push(entry.mod_id.clone());
            }
        }
    }

    // Graph edge u -> v means `u` must load BEFORE `v`.
    let mut adj: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut indeg: BTreeMap<String, usize> = BTreeMap::new();
    for id in &all_mods {
        adj.insert(id.clone(), BTreeSet::new());
        indeg.insert(id.clone(), 0);
    }

    for m in manifests {
        for (mod_id, deps) in &m.dependencies {
            if !known_set.contains(mod_id) {
                continue;
            }
            for dep in deps {
                if !dep.side.matches_server(is_dedicated_server) {
                    continue;
                }
                let target = &dep.mod_id;
                match dep.dep_type {
                    DependencyType::Incompatible => {
                        if known_set.contains(target) || builtin.contains(target.as_str()) {
                            return Err(format!(
                                "mod `{mod_id}` declares incompatibility with loaded mod `{target}`"
                            ));
                        }
                    }
                    DependencyType::Required => {
                        if !known_set.contains(target) && !builtin.contains(target.as_str()) {
                            return Err(format!(
                                "mod `{mod_id}` is missing required dependency `{target}` ({})",
                                dep.version_range
                            ));
                        }
                    }
                    DependencyType::Optional | DependencyType::Discouraged => {}
                }

                if known_set.contains(target) && target != mod_id {
                    match dep.ordering {
                        // `ordering = "BEFORE"` in NeoForge means THIS mod (`mod_id`) loads BEFORE `target`!
                        DependencyOrdering::Before => {
                            if adj.get_mut(mod_id).unwrap().insert(target.clone()) {
                                *indeg.get_mut(target).unwrap() += 1;
                            }
                        }
                        // `ordering = "AFTER"` (or default on required dep) means `mod_id` loads AFTER `target`.
                        DependencyOrdering::After => {
                            if adj.get_mut(target).unwrap().insert(mod_id.clone()) {
                                *indeg.get_mut(mod_id).unwrap() += 1;
                            }
                        }
                        DependencyOrdering::None if dep.dep_type == DependencyType::Required => {
                            if adj.get_mut(target).unwrap().insert(mod_id.clone()) {
                                *indeg.get_mut(mod_id).unwrap() += 1;
                            }
                        }
                        DependencyOrdering::None => {}
                    }
                }
            }
        }
    }

    let mut queue: VecDeque<String> = all_mods
        .iter()
        .filter(|id| indeg.get(*id) == Some(&0))
        .cloned()
        .collect();

    let mut ordered = Vec::with_capacity(all_mods.len());
    while let Some(u) = queue.pop_front() {
        ordered.push(u.clone());
        if let Some(neighbors) = adj.get(&u) {
            for v in neighbors {
                let d = indeg.get_mut(v).unwrap();
                *d -= 1;
                if *d == 0 {
                    queue.push_back(v.clone());
                }
            }
        }
    }

    if ordered.len() != all_mods.len() {
        return Err("cyclic mod dependency ordering detected".into());
    }
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_neoforge_mods_toml_and_sorts_dependencies() {
        let core_toml = r#"
modLoader = "javafml"
loaderVersion = "[4,)"
license = "MIT"

[[mods]]
modId = "kinetic_core"
version = "${file.jarVersion}"
displayName = "Kinetic Core"

[[mixins]]
config = "kinetic_core.mixins.json"
"#;
        let addon_toml = r#"
modLoader = "javafml"
loaderVersion = "[4,)"
license = "MIT"

[[mods]]
modId = "kinetic_addon"
version = "1.2.0"
displayName = "Kinetic Addon"

[[dependencies.kinetic_addon]]
modId = "neoforge"
type = "required"
versionRange = "[21.1,)"
ordering = "NONE"
side = "BOTH"

[[dependencies.kinetic_addon]]
modId = "kinetic_core"
type = "required"
versionRange = "[1.0,)"
ordering = "AFTER"
side = "BOTH"
"#;
        let core = ModsToml::parse(core_toml, Some("1.5.2")).unwrap();
        let addon = ModsToml::parse(addon_toml, None).unwrap();
        assert_eq!(core.mods[0].version, "1.5.2");
        assert_eq!(core.mixins[0].config, "kinetic_core.mixins.json");

        // Even if addon is passed first, topological sort puts `kinetic_core` first!
        let order = resolve_mod_load_order(&[&addon, &core], false).unwrap();
        assert_eq!(order, vec!["kinetic_core", "kinetic_addon"]);
    }
}
