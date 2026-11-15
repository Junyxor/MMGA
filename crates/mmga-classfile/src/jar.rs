//! `.jar` archive reader (`ModJar`), nested Jar-in-Jar (`META-INF/jarjar/*.jar`) loader,
//! NeoForge `@Mod` & `@EventBusSubscriber` scanner, and resource/data extractor.

use crate::class::ClassFile;
use crate::metadata::{parse_manifest_mf, JarJarMetadata, MixinConfig, ModSide, ModsToml};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::Path;

/// Discovered `@Mod("modid")` class inside a jar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredModEntrypoint {
    pub mod_id: String,
    pub class_name: String,
    pub dist_restriction: ModSide,
    pub constructor_descriptors: Vec<String>,
}

/// Which NeoForge event bus `@EventBusSubscriber` targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubscriberBusTarget {
    /// `NeoForge.EVENT_BUS` (default in NeoForge 21.1+ when `bus` is omitted or `Bus.GAME` / `Bus.FORGE`).
    Game,
    /// The mod's own `IEventBus` (`bus = EventBusSubscriber.Bus.MOD`, or auto-routed when event implements `IModBusEvent` in NeoForge 21.1.90+/26.3).
    Mod,
    /// Auto-route per `@SubscribeEvent` method parameter (`IModBusEvent` -> `Mod`, else `Game`).
    Auto,
}

/// Discovered `@EventBusSubscriber` class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredSubscriber {
    pub class_name: String,
    pub mod_id: Option<String>,
    pub bus: SubscriberBusTarget,
    pub side: ModSide,
    pub methods: Vec<DiscoveredSubscribeMethod>,
}

/// Discovered `@SubscribeEvent` method on a class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredSubscribeMethod {
    pub method_name: String,
    pub method_desc: String,
    pub event_class: String,
    pub is_static: bool,
    pub priority: String,
    pub receive_canceled: bool,
}

/// Parsed NeoForge `.jar` file (with optional nested Jar-in-Jar classes merged).
#[derive(Debug, Clone)]
pub struct ModJar {
    pub source_name: String,
    pub manifest_headers: BTreeMap<String, String>,
    pub mods_toml: Option<ModsToml>,
    /// `mods.toml` files from Jar-in-Jar nested archives. Their classes are merged into
    /// [`Self::classes`]; the manifests stay separate so each embedded mod keeps its id.
    pub nested_manifests: Vec<ModsToml>,
    /// Internal class name (`"com/example/MyMod"`) -> parsed `ClassFile`.
    pub classes: BTreeMap<String, ClassFile>,
    /// Config path (`"mymod.mixins.json"`) -> parsed `MixinConfig`.
    pub mixin_configs: BTreeMap<String, MixinConfig>,
    /// `META-INF/services/<spi>` -> implementation internal class names.
    pub services: BTreeMap<String, Vec<String>>,
    /// Non-class resources (`assets/...`, `data/...`, configs, etc.).
    pub resources: BTreeMap<String, Vec<u8>>,
    /// Nested jar paths unpacked from `META-INF/jarjar/`.
    pub nested_jars: Vec<String>,
}

impl ModJar {
    /// Loads a `.jar` from disk.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let p = path.as_ref();
        let name = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown.jar")
            .to_owned();
        let bytes =
            std::fs::read(p).map_err(|e| format!("failed to read jar `{}`: {e}", p.display()))?;
        Self::from_bytes(name, &bytes)
    }

    /// Loads a `.jar` (and any nested `META-INF/jarjar/*.jar` archives) from memory.
    pub fn from_bytes(source_name: impl Into<String>, bytes: &[u8]) -> Result<Self, String> {
        let mut jar = ModJar {
            source_name: source_name.into(),
            manifest_headers: BTreeMap::new(),
            mods_toml: None,
            nested_manifests: Vec::new(),
            classes: BTreeMap::new(),
            mixin_configs: BTreeMap::new(),
            services: BTreeMap::new(),
            resources: BTreeMap::new(),
            nested_jars: Vec::new(),
        };
        jar.ingest_zip_bytes(bytes, true)?;
        Ok(jar)
    }

    fn ingest_zip_bytes(&mut self, bytes: &[u8], is_root: bool) -> Result<(), String> {
        let reader = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|e| format!("{}: invalid zip/jar archive: {e}", self.source_name))?;

        let mut raw_entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for idx in 0..archive.len() {
            let mut file = archive
                .by_index(idx)
                .map_err(|e| format!("{}: zip entry #{idx} error: {e}", self.source_name))?;
            if file.is_dir() {
                continue;
            }
            let entry_name = file.name().trim_start_matches('/').to_owned();
            // Reject path traversal
            if entry_name.split('/').any(|seg| seg == "..") {
                continue;
            }
            let mut buf = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buf)
                .map_err(|e| format!("{}: failed reading `{entry_name}`: {e}", self.source_name))?;
            raw_entries.insert(entry_name, buf);
        }

        let manifest_headers = raw_entries
            .get("META-INF/MANIFEST.MF")
            .map(|bytes| parse_manifest_mf(&String::from_utf8_lossy(bytes)))
            .unwrap_or_default();
        let jar_version = manifest_headers.get("Implementation-Version").cloned();
        if is_root {
            self.manifest_headers = manifest_headers;
        }
        if let Some(toml_bytes) = raw_entries
            .get("META-INF/neoforge.mods.toml")
            .or_else(|| raw_entries.get("META-INF/mods.toml"))
        {
            let parsed =
                ModsToml::parse(&String::from_utf8_lossy(toml_bytes), jar_version.as_deref())?;
            if is_root {
                self.mods_toml = Some(parsed);
            } else {
                self.nested_manifests.push(parsed);
            }
        }

        // Collect nested Jar-in-Jar paths from `META-INF/jarjar/metadata.json` or `META-INF/jarjar/*.jar`
        let mut nested_paths = Vec::new();
        if let Some(jj_bytes) = raw_entries.get("META-INF/jarjar/metadata.json") {
            if let Ok(meta) = serde_json::from_slice::<JarJarMetadata>(jj_bytes) {
                for j in meta.jars {
                    nested_paths.push(j.path);
                }
            }
        }
        for k in raw_entries.keys() {
            if k.starts_with("META-INF/jarjar/") && k.ends_with(".jar") && !nested_paths.contains(k)
            {
                nested_paths.push(k.clone());
            }
        }

        for path in nested_paths {
            if let Some(nested_bytes) = raw_entries.remove(&path) {
                self.nested_jars.push(path);
                // Nested library classes are loaded first so root jar classes take precedence.
                self.ingest_zip_bytes(&nested_bytes, false)?;
            }
        }

        let declared_mixin_configs: Vec<String> = self
            .mods_toml
            .as_ref()
            .map(|m| m.mixins.iter().map(|x| x.config.clone()).collect())
            .unwrap_or_default();

        for (entry_path, data) in raw_entries {
            if entry_path.ends_with(".class") && entry_path != "module-info.class" {
                // Skip multi-release Java 22+ duplicates if base class exists unless needed
                if entry_path.starts_with("META-INF/versions/") {
                    continue;
                }
                let cf = ClassFile::parse(&data)
                    .map_err(|e| format!("{}:{entry_path}: {e}", self.source_name))?;
                self.classes.insert(cf.name.clone(), cf);
                continue;
            }

            if let Some(spi) = entry_path.strip_prefix("META-INF/services/") {
                if !spi.is_empty() && !spi.contains('/') {
                    let impls = self.services.entry(spi.replace('.', "/")).or_default();
                    for line in String::from_utf8_lossy(&data).lines() {
                        let trimmed = line.split('#').next().unwrap_or("").trim();
                        if !trimmed.is_empty() {
                            let internal = trimmed.replace('.', "/");
                            if !impls.contains(&internal) {
                                impls.push(internal);
                            }
                        }
                    }
                }
            }

            if (entry_path.ends_with(".mixins.json")
                || entry_path.starts_with("mixins.") && entry_path.ends_with(".json")
                || declared_mixin_configs.contains(&entry_path))
                && is_root
            {
                if let Ok(cfg) = MixinConfig::parse(&String::from_utf8_lossy(&data)) {
                    self.mixin_configs.insert(entry_path.clone(), cfg);
                }
            }

            self.resources.entry(entry_path).or_insert(data);
        }

        Ok(())
    }

    /// Primary mod ID of this jar, if declared in `neoforge.mods.toml` or via `@Mod`.
    pub fn primary_mod_id(&self) -> Option<String> {
        if let Some(toml) = &self.mods_toml {
            if let Some(first) = toml.mods.first() {
                return Some(first.mod_id.clone());
            }
        }
        self.discover_entrypoints()
            .into_iter()
            .next()
            .map(|e| e.mod_id)
    }

    /// Scans all classes for `@Mod("id")` entrypoints (including NeoForge 21.1+ `@Mod(value = "id", dist = Dist.CLIENT)`).
    pub fn discover_entrypoints(&self) -> Vec<DiscoveredModEntrypoint> {
        let mut out = Vec::new();
        for cf in self.classes.values() {
            let Some(ann) = cf
                .annotation("Lnet/neoforged/fml/common/Mod;")
                .or_else(|| cf.annotation("Lnet/minecraftforge/fml/common/Mod;"))
            else {
                continue;
            };
            let Some(mod_id) = ann.get_str("value").filter(|s| !s.is_empty()) else {
                continue;
            };
            let dist = ann
                .get("dist")
                .map(|v| {
                    let items = v.to_string_list();
                    let has_client = items.iter().any(|x| x == "CLIENT");
                    let has_server = items.iter().any(|x| x == "DEDICATED_SERVER");
                    match (has_client, has_server) {
                        (true, false) => ModSide::Client,
                        (false, true) => ModSide::Server,
                        _ => ModSide::Both,
                    }
                })
                .unwrap_or(ModSide::Both);

            let constructor_descriptors = cf
                .methods
                .iter()
                .filter(|m| m.name == "<init>")
                .map(|m| m.desc.clone())
                .collect();

            out.push(DiscoveredModEntrypoint {
                mod_id: mod_id.to_owned(),
                class_name: cf.name.clone(),
                dist_restriction: dist,
                constructor_descriptors,
            });
        }
        out
    }

    /// Scans all classes for `@EventBusSubscriber` and extracts their static `@SubscribeEvent` methods.
    pub fn discover_event_bus_subscribers(&self) -> Vec<DiscoveredSubscriber> {
        let mut out = Vec::new();
        for cf in self.classes.values() {
            let Some(ann) = cf
                .annotation("Lnet/neoforged/fml/common/EventBusSubscriber;")
                .or_else(|| cf.annotation("Lnet/neoforged/fml/common/Mod$EventBusSubscriber;"))
                .or_else(|| {
                    cf.annotation("Lnet/minecraftforge/fml/common/Mod$EventBusSubscriber;")
                })
            else {
                continue;
            };

            let mod_id = ann.get_str("modid").map(str::to_owned);
            let bus = match ann.get_str("bus") {
                Some("MOD") => SubscriberBusTarget::Mod,
                Some("GAME" | "FORGE") => SubscriberBusTarget::Game,
                None => SubscriberBusTarget::Auto,
                _ => SubscriberBusTarget::Auto,
            };
            let side = ann
                .get("value")
                .or_else(|| ann.get("dist"))
                .map(|v| {
                    let items = v.to_string_list();
                    let has_client = items.iter().any(|x| x == "CLIENT");
                    let has_server = items.iter().any(|x| x == "DEDICATED_SERVER");
                    match (has_client, has_server) {
                        (true, false) => ModSide::Client,
                        (false, true) => ModSide::Server,
                        _ => ModSide::Both,
                    }
                })
                .unwrap_or(ModSide::Both);

            let methods = scan_subscribe_methods(cf, true);
            out.push(DiscoveredSubscriber {
                class_name: cf.name.clone(),
                mod_id,
                bus,
                side,
                methods,
            });
        }
        out
    }
}

/// Scans a `ClassFile` for `@SubscribeEvent` methods (`only_static = true` for `Class`/subscriber registration,
/// `false` when scanning an object instance passed to `IEventBus.register(instance)`).
pub fn scan_subscribe_methods(cf: &ClassFile, only_static: bool) -> Vec<DiscoveredSubscribeMethod> {
    let mut out = Vec::new();
    for m in &cf.methods {
        if only_static && !m.is_static() {
            continue;
        }
        let Some(sub) = m
            .annotation("Lnet/neoforged/bus/api/SubscribeEvent;")
            .or_else(|| m.annotation("Lnet/minecraftforge/eventbus/api/SubscribeEvent;"))
        else {
            continue;
        };
        let Ok((params, _)) = crate::insn::parse_method_descriptor(&m.desc) else {
            continue;
        };
        if params.len() != 1 {
            continue;
        }
        let Some(event_class) = params[0].internal_name() else {
            continue;
        };
        let priority = sub.get_str("priority").unwrap_or("NORMAL").to_owned();
        let receive_canceled = sub.get_bool("receiveCanceled").unwrap_or(false);
        out.push(DiscoveredSubscribeMethod {
            method_name: m.name.clone(),
            method_desc: m.desc.clone(),
            event_class,
            is_static: m.is_static(),
            priority,
            receive_canceled,
        });
    }
    out
}
