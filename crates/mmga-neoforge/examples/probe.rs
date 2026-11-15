//! Load unmodified mod jars and report how far MMGA gets.
//!
//! Usage: `cargo run -p mmga-neoforge --example probe -- --dedicated-server <jar>...`
//! Jars are read from the paths you pass. This example does not bundle any mod.

use mmga_classfile::{ClassFile, ModJar};
use mmga_neoforge::PreparedLoad;
use mmga_runtime::Rt;
use std::env;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use std::time::Instant;

fn main() {
    let mut dedicated = true;
    let mut paths = Vec::new();
    for arg in env::args().skip(1) {
        if arg == "--client" {
            dedicated = false;
        } else if arg == "--dedicated-server" {
            dedicated = true;
        } else {
            paths.push(arg);
        }
    }
    if paths.is_empty() {
        eprintln!("usage: probe [--dedicated-server|--client] <mod.jar>...");
        std::process::exit(2);
    }

    println!(
        "side={}",
        if dedicated {
            "dedicated-server"
        } else {
            "client"
        }
    );
    let mut jars = Vec::new();
    for path in &paths {
        let started = Instant::now();
        match load_one(Path::new(path)) {
            Ok(jar) => {
                println!(
                    "opened {} in {:?} classes={} mods={} nested={:?}",
                    jar.source_name,
                    started.elapsed(),
                    jar.classes.len(),
                    jar.mods_toml
                        .as_ref()
                        .map(|m| {
                            m.mods
                                .iter()
                                .map(|e| e.mod_id.clone())
                                .collect::<Vec<_>>()
                                .join(",")
                        })
                        .unwrap_or_default(),
                    jar.nested_jars
                );
                if let Some(toml) = &jar.mods_toml {
                    for (id, deps) in &toml.dependencies {
                        for dep in deps {
                            println!(
                                "  dep {id} -> {} {:?} range={} side={:?} order={:?}",
                                dep.mod_id, dep.dep_type, dep.version_range, dep.side, dep.ordering
                            );
                        }
                    }
                }
                let eps = jar.discover_entrypoints();
                println!("  @Mod entrypoints: {}", eps.len());
                for ep in eps.iter().take(8) {
                    println!(
                        "    {} class={} dist={:?} ctors={:?}",
                        ep.mod_id, ep.class_name, ep.dist_restriction, ep.constructor_descriptors
                    );
                }
                jars.push(jar);
            }
            Err(err) => {
                println!("OPEN FAILED {path}: {err}");
            }
        }
    }

    if jars.is_empty() {
        println!("no jar opened; not attempting lifecycle");
        return;
    }

    match PreparedLoad::from_jars(jars, dedicated) {
        Err(err) => println!("LOAD ORDER FAILED: {err}"),
        Ok(mut prepared) => {
            println!(
                "load order: {}",
                prepared
                    .loader
                    .mods
                    .iter()
                    .map(|m| format!(
                        "{}@{} entry={}",
                        m.mod_id,
                        m.version,
                        m.entry_class.as_deref().unwrap_or("-")
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let mut rt = Rt::new();
            match prepared.bootstrap(&mut rt) {
                Ok(()) => {
                    println!(
                        "bootstrap returned Ok phase={:?} trace={:?} mod_entries={} configs={} commands={} listeners={} loading_issues={} events_posted={} callbacks={} unmapped={}",
                        prepared.loader.phase,
                        prepared.loader.trace,
                        prepared.loader.state.registries.total_mod_entries(),
                        prepared.loader.registered_configs.len(),
                        prepared.loader.state.commands.roots.len(),
                        prepared.loader.state.bus.listener_count(),
                        prepared.loader.loading_issues.len(),
                        prepared.loader.state.bus.events_posted,
                        prepared.loader.state.bus.callbacks_invoked,
                        rt.telemetry.unmapped_calls.len()
                    );
                    for issue in &prepared.loader.loading_issues {
                        println!(
                            "  loading_issue [{}] mod={:?} key={}: {}",
                            issue.severity, issue.mod_id, issue.translation_key, issue.message
                        );
                    }
                    for log in rt.telemetry.logs.iter().take(15) {
                        println!("  log: {log}");
                    }
                    match prepared.loader.tick_server(&mut rt, &prepared.linked, 1) {
                        Ok(()) => println!(
                            "tick_server(1) returned Ok game_time={}",
                            prepared.loader.state.world.game_time
                        ),
                        Err(err) => println!("TICK FAILED: {err}"),
                    }
                    let mut keys: Vec<_> = rt.telemetry.unmapped_calls.keys().cloned().collect();
                    keys.sort();
                    for key in keys.iter().take(30) {
                        println!("  unmapped {} x{}", key, rt.telemetry.unmapped_calls[key]);
                    }
                    println!(
                        "NOTE: Ok only means the loader finished its phase list. It is not gameplay, world simulation, or rendering."
                    );
                }
                Err(err) => {
                    println!(
                        "BOOTSTRAP FAILED at {:?} (trace={:?}, mod_entries={}, configs={}, listeners={}): {err}",
                        prepared.loader.phase,
                        prepared.loader.trace,
                        prepared.loader.state.registries.total_mod_entries(),
                        prepared.loader.registered_configs.len(),
                        prepared.loader.state.bus.listener_count(),
                    );
                    for log in rt.telemetry.logs.iter().take(15) {
                        println!("  log: {log}");
                    }
                }
            }
        }
    }
}

fn load_one(path: &Path) -> Result<ModJar, String> {
    let bytes = fs::read(path).map_err(|e| format!("read: {e}"))?;
    let mut class_errors = 0usize;
    let mut first_error = None;
    scan_zip_classes(
        &bytes,
        &mut class_errors,
        &mut first_error,
        path.display().to_string(),
    )?;
    if let Some(err) = &first_error {
        println!("  classfile scan: {class_errors} failures, first: {err}");
    } else {
        println!("  classfile scan: all scanned class files parsed");
    }
    ModJar::from_bytes(
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("mod.jar"),
        &bytes,
    )
}

fn scan_zip_classes(
    bytes: &[u8],
    errors: &mut usize,
    first: &mut Option<String>,
    label: String,
) -> Result<(), String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("{label}: zip: {e}"))?;
    for idx in 0..archive.len() {
        let mut file = archive
            .by_index(idx)
            .map_err(|e| format!("{label}: entry {idx}: {e}"))?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_owned();
        if name.split('/').any(|seg| seg == "..") {
            continue;
        }
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|e| format!("{label}:{name}: {e}"))?;
        if name.ends_with(".class")
            && !name.starts_with("META-INF/versions/")
            && name != "module-info.class"
        {
            if let Err(err) = ClassFile::parse(&buf) {
                *errors += 1;
                if first.is_none() {
                    *first = Some(format!("{label}:{name}: {err}"));
                }
            }
        } else if name.starts_with("META-INF/jarjar/") && name.ends_with(".jar") {
            scan_zip_classes(&buf, errors, first, format!("{label}:{name}"))?;
        }
    }
    Ok(())
}
