//! Throughput of the native event bus, registry lookup, and a full server tick.
//!
//! `cargo bench -p mmga-neoforge` runs this in release mode. The numbers are the
//! Rust host path, not a comparison against a JVM NeoForge server.

use mmga_classfile::{ModJar, ModsToml};
use mmga_neoforge::{BusId, EventCallback, EventPriority, NeoForgeState, PreparedLoad};
use mmga_runtime::{ExecCtx, LinkedSpace, NullHost, Rt, V};
use std::collections::BTreeMap;
use std::time::Instant;

fn main() {
    bench_event_dispatch();
    bench_registry_lookup();
    bench_server_ticks();
}

fn bench_event_dispatch() {
    let mut state = NeoForgeState::new();
    for priority in [
        EventPriority::Lowest,
        EventPriority::Low,
        EventPriority::Normal,
        EventPriority::High,
        EventPriority::Highest,
        EventPriority::Normal,
        EventPriority::High,
        EventPriority::Low,
    ] {
        state
            .bus
            .register_listener(
                BusId::Game,
                "net/neoforged/neoforge/event/tick/ServerTickEvent$Post".into(),
                priority,
                false,
                V::NULL,
                EventCallback::Native(count_hit),
            )
            .unwrap();
    }
    let mut rt = Rt::new();
    let linked = LinkedSpace::new();
    let mut host = NullHost;
    let event = rt.new_instance("net/neoforged/neoforge/event/tick/ServerTickEvent$Post", 0);
    rt.put_field(event, "$canceled", V::I(0));
    rt.put_field(event, "hits", V::I(0));

    let posts = 200_000u32;
    let started = Instant::now();
    for _ in 0..posts {
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let _ = state.bus.post(
            &mut ctx,
            &BusId::Game,
            "net/neoforged/neoforge/event/tick/ServerTickEvent$Post",
            event,
        );
    }
    let elapsed = started.elapsed();
    let rate = posts as f64 / elapsed.as_secs_f64();
    println!("event bus: {posts} posts x 8 listeners in {elapsed:.3?} ({rate:.0} posts/s)");
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "event dispatch slower than 40k posts/s: {elapsed:?}"
    );
    assert_eq!(
        rt.get_field(event, "hits").i(),
        (posts as i32).wrapping_mul(8)
    );
}

fn count_hit(rt: &mut Rt, event: V) {
    let hits = rt.get_field(event, "hits").i();
    rt.put_field(event, "hits", V::I(hits.wrapping_add(1)));
}

fn bench_registry_lookup() {
    let mut rt = Rt::new();
    let mut state = NeoForgeState::new();
    for i in 0..1_000 {
        let value = rt.new_instance("net/minecraft/world/level/block/Block", 0);
        state.registries.register_direct(
            &mut rt,
            "minecraft:block",
            &format!("bench:block_{i}"),
            "bench",
            value,
            Default::default(),
        );
    }
    let lookups = 200_000u32;
    let started = Instant::now();
    let mut found = 0usize;
    for i in 0..lookups {
        let id = format!("bench:block_{}", i % 1_000);
        if state.registries.lookup("block", &id).is_some() {
            found += 1;
        }
    }
    let elapsed = started.elapsed();
    let rate = lookups as f64 / elapsed.as_secs_f64();
    println!("registry: {lookups} lookups in {elapsed:.3?} ({rate:.0} lookups/s), found {found}");
    assert_eq!(found, lookups as usize);
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "registry lookup too slow: {elapsed:?}"
    );
}

fn bench_server_ticks() {
    let jar = ModJar {
        source_name: "bench.jar".into(),
        manifest_headers: BTreeMap::new(),
        mods_toml: Some(
            ModsToml::parse(
                r#"
                modLoader = "javafml"
                loaderVersion = "[4,)"
                license = "MIT"
                [[mods]]
                modId = "bench"
                version = "0.0.0"
                "#,
                None,
            )
            .unwrap(),
        ),
        classes: BTreeMap::new(),
        mixin_configs: BTreeMap::new(),
        services: BTreeMap::new(),
        resources: BTreeMap::new(),
        nested_jars: Vec::new(),
    };
    let mut prepared = PreparedLoad::from_jars(vec![jar], true).unwrap();
    let mut rt = Rt::new();
    prepared.bootstrap(&mut rt).unwrap();

    let ticks = 5_000u32;
    let started = Instant::now();
    prepared
        .loader
        .tick_server(&mut rt, &prepared.linked, ticks)
        .unwrap();
    let elapsed = started.elapsed();
    let rate = ticks as f64 / elapsed.as_secs_f64();
    println!(
        "server tick: {ticks} ticks (pre + post events, young gc) in {elapsed:.3?} ({rate:.0} ticks/s)"
    );
    assert_eq!(prepared.loader.state.world.game_time, i64::from(ticks));
    assert!(
        elapsed.as_secs_f64() < 8.0,
        "server tick too slow: {elapsed:?}"
    );
}
