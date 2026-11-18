//! Stopwatch state and read order verified against Minecraft 1.21.1's Guava
//! 32.1.2-jre artifact (SHA-1 5e64ec7e056456bef3a4bc4c6fdaef71e8ab6318).

use crate::{invoke_member, read_static_field, ExecCtx, Rt, V};
use mmga_classfile::{op, MemberRef};
use std::sync::OnceLock;
use std::time::Instant;

const STOPWATCH: &str = "com/google/common/base/Stopwatch";
const TICKER: &str = "com/google/common/base/Ticker";
const SYSTEM_TICKER: &str = "com/google/common/base/Ticker$1";
const TIME_UNIT: &str = "java/util/concurrent/TimeUnit";
const TIME_UNIT_DESC: &str = "Ljava/util/concurrent/TimeUnit;";

const UNITS: [(&str, i64); 7] = [
    ("NANOSECONDS", 1),
    ("MICROSECONDS", 1_000),
    ("MILLISECONDS", 1_000_000),
    ("SECONDS", 1_000_000_000),
    ("MINUTES", 60_000_000_000),
    ("HOURS", 3_600_000_000_000),
    ("DAYS", 86_400_000_000_000),
];

pub(crate) fn get_static(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != TIME_UNIT || desc != TIME_UNIT_DESC {
        return None;
    }
    let (ordinal, &(name, nanos)) = UNITS.iter().enumerate().find(|(_, unit)| unit.0 == name)?;
    if let Some(value) = rt.get_static(TIME_UNIT, name) {
        return Some(value);
    }
    let value = rt.new_instance(TIME_UNIT, 0);
    let java_name = rt.intern_str(name);
    rt.put_field(value, "$name", java_name);
    rt.put_field(value, "$ordinal", V::I(ordinal as i32));
    rt.put_field(value, "$nanosPerUnit", V::J(nanos));
    rt.put_static(TIME_UNIT, name, value);
    Some(value)
}

fn system_ticker(rt: &mut Rt) -> V {
    if let Some(ticker) = rt.get_static(TICKER, "SYSTEM_TICKER") {
        return ticker;
    }
    let ticker = rt.new_instance(SYSTEM_TICKER, 0);
    rt.put_static(TICKER, "SYSTEM_TICKER", ticker);
    ticker
}

fn init(rt: &mut Rt, watch: V, ticker: V) -> V {
    if ticker.is_null() {
        return rt.throw_exception("java/lang/NullPointerException", "ticker");
    }
    rt.put_field(watch, "ticker", ticker);
    rt.put_field(watch, "isRunning", V::FALSE);
    rt.put_field(watch, "elapsedNanos", V::J(0));
    rt.put_field(watch, "startTick", V::J(0));
    V::Void
}

fn ticker_read(ctx: &mut ExecCtx<'_>, watch: V) -> Option<i64> {
    let ticker = ctx.rt.get_field(watch, "ticker");
    let result = invoke_member(
        ctx,
        op::INVOKEVIRTUAL,
        &MemberRef::new(TICKER, "read", "()J"),
        &[ticker],
    );
    ctx.rt.pending_exception.is_none().then(|| result.j())
}

fn start(ctx: &mut ExecCtx<'_>, watch: V) -> V {
    if ctx.rt.get_field(watch, "isRunning") == V::TRUE {
        return ctx.rt.throw_exception(
            "java/lang/IllegalStateException",
            "This stopwatch is already running.",
        );
    }
    ctx.rt.put_field(watch, "isRunning", V::TRUE);
    let Some(tick) = ticker_read(ctx, watch) else {
        return V::NULL;
    };
    ctx.rt.put_field(watch, "startTick", V::J(tick));
    watch
}

fn stop(ctx: &mut ExecCtx<'_>, watch: V) -> V {
    let Some(tick) = ticker_read(ctx, watch) else {
        return V::NULL;
    };
    if ctx.rt.get_field(watch, "isRunning") != V::TRUE {
        return ctx.rt.throw_exception(
            "java/lang/IllegalStateException",
            "This stopwatch is already stopped.",
        );
    }
    ctx.rt.put_field(watch, "isRunning", V::FALSE);
    let duration = tick.wrapping_sub(ctx.rt.get_field(watch, "startTick").j());
    let accumulated = ctx.rt.get_field(watch, "elapsedNanos").j();
    ctx.rt.put_field(
        watch,
        "elapsedNanos",
        V::J(accumulated.wrapping_add(duration)),
    );
    watch
}

fn elapsed(ctx: &mut ExecCtx<'_>, watch: V, unit: V) -> V {
    let current = if ctx.rt.get_field(watch, "isRunning") == V::TRUE {
        let Some(tick) = ticker_read(ctx, watch) else {
            return V::NULL;
        };
        tick.wrapping_sub(ctx.rt.get_field(watch, "startTick").j())
    } else {
        0
    };
    let nanos = current.wrapping_add(ctx.rt.get_field(watch, "elapsedNanos").j());
    let source = read_static_field(ctx, TIME_UNIT, "NANOSECONDS", TIME_UNIT_DESC);
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    invoke_member(
        ctx,
        op::INVOKEVIRTUAL,
        &MemberRef::new(TIME_UNIT, "convert", "(JLjava/util/concurrent/TimeUnit;)J"),
        &[unit, V::J(nanos), source],
    )
}

fn convert(rt: &mut Rt, value: V, source: V, target: V) -> V {
    if source.is_null() {
        return rt.throw_exception("java/lang/NullPointerException", "sourceUnit");
    }
    let scale = |unit| {
        let nanos = rt.get_field(unit, "$nanosPerUnit").j();
        (rt.class_of(unit) == Some(TIME_UNIT) && nanos > 0).then_some(i128::from(nanos))
    };
    let (Some(source), Some(target)) = (scale(source), scale(target)) else {
        return rt.throw_exception(
            "java/lang/ClassCastException",
            "Expected TimeUnit constants",
        );
    };
    let result = i128::from(value.j()) * source / target;
    V::J(result.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64)
}

pub(crate) fn call(
    ctx: &mut ExecCtx<'_>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if is_static {
        return match (owner, name, desc) {
            (TICKER, "systemTicker", "()Lcom/google/common/base/Ticker;") => {
                Some(system_ticker(ctx.rt))
            }
            (
                STOPWATCH,
                "createUnstarted" | "createStarted",
                "()Lcom/google/common/base/Stopwatch;",
            )
            | (
                STOPWATCH,
                "createUnstarted" | "createStarted",
                "(Lcom/google/common/base/Ticker;)Lcom/google/common/base/Stopwatch;",
            ) => {
                let ticker = if desc.starts_with("()") {
                    system_ticker(ctx.rt)
                } else {
                    *args.first()?
                };
                let watch = ctx.rt.new_instance(STOPWATCH, 0);
                init(ctx.rt, watch, ticker);
                Some(if ctx.rt.pending_exception.is_some() {
                    V::NULL
                } else if name == "createStarted" {
                    start(ctx, watch)
                } else {
                    watch
                })
            }
            _ => None,
        };
    }
    let receiver = *args.first()?;
    Some(match (owner, name, desc) {
        (TICKER, "<init>", "()V") => V::Void,
        (TICKER | SYSTEM_TICKER, "read", "()J")
            if ctx.rt.class_of(receiver) == Some(SYSTEM_TICKER) =>
        {
            static EPOCH: OnceLock<Instant> = OnceLock::new();
            V::J(EPOCH.get_or_init(Instant::now).elapsed().as_nanos() as i64)
        }
        (TIME_UNIT, "convert", "(JLjava/util/concurrent/TimeUnit;)J") => {
            convert(ctx.rt, *args.get(1)?, *args.get(2)?, receiver)
        }
        (STOPWATCH, "<init>", "()V") => {
            let ticker = system_ticker(ctx.rt);
            init(ctx.rt, receiver, ticker)
        }
        (STOPWATCH, "<init>", "(Lcom/google/common/base/Ticker;)V") => {
            init(ctx.rt, receiver, *args.get(1)?)
        }
        (STOPWATCH, "start", "()Lcom/google/common/base/Stopwatch;") => start(ctx, receiver),
        (STOPWATCH, "stop", "()Lcom/google/common/base/Stopwatch;") => stop(ctx, receiver),
        (STOPWATCH, "reset", "()Lcom/google/common/base/Stopwatch;") => {
            ctx.rt.put_field(receiver, "elapsedNanos", V::J(0));
            ctx.rt.put_field(receiver, "isRunning", V::FALSE);
            receiver
        }
        (STOPWATCH, "isRunning", "()Z") => ctx.rt.get_field(receiver, "isRunning"),
        (STOPWATCH, "elapsed", "(Ljava/util/concurrent/TimeUnit;)J") => {
            elapsed(ctx, receiver, *args.get(1)?)
        }
        (STOPWATCH, "elapsed", "()Ljava/time/Duration;") => ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Stopwatch.elapsed() requires java.time.Duration",
        ),
        (STOPWATCH | "java/lang/Object", "toString", "()Ljava/lang/String;")
            if ctx.rt.class_of(receiver) == Some(STOPWATCH) =>
        {
            ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Stopwatch.toString() formatting is not implemented",
            )
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LinkedSpace, NativeClassDef, NativeModuleRegistry, NullHost};

    const FIXTURE: &str = "test/ManualTicker";
    const WATCH_RETURN: &str = "()Lcom/google/common/base/Stopwatch;";
    const WITH_TICKER: &str = "(Lcom/google/common/base/Ticker;)Lcom/google/common/base/Stopwatch;";
    const ELAPSED: &str = "(Ljava/util/concurrent/TimeUnit;)J";

    fn read(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let ticker = args[0];
        let count = ctx.rt.get_field(ticker, "reads").i();
        ctx.rt.put_field(ticker, "reads", V::I(count + 1));
        let error = ctx.rt.get_field(ticker, "failure");
        if !error.is_null() {
            ctx.rt.pending_exception = Some(error.r());
            return V::NULL;
        }
        ctx.rt.get_field(ticker, "tick")
    }

    fn method(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, args: &[V]) -> V {
        let opcode = if name.starts_with("create") {
            op::INVOKESTATIC
        } else {
            op::INVOKEVIRTUAL
        };
        invoke_member(ctx, opcode, &MemberRef::new(STOPWATCH, name, desc), args)
    }

    #[test]
    fn deterministic_ticker_observes_guava_read_order_and_wrapping_transitions() {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[NativeClassDef {
                name: FIXTURE,
                superclass: Some(TICKER),
                interfaces: &[],
                is_interface: false,
                methods: &[("read.()J", read, false, false)],
            }],
            lambdas: &[],
        });
        let mut rt = Rt::new();
        let mut host = NullHost;
        let ticker = rt.new_instance(FIXTURE, 0);
        rt.put_field(ticker, "tick", V::J(i64::MAX - 499_999));
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let milliseconds = read_static_field(&mut ctx, TIME_UNIT, "MILLISECONDS", TIME_UNIT_DESC);
        let nanos = read_static_field(&mut ctx, TIME_UNIT, "NANOSECONDS", TIME_UNIT_DESC);
        assert_eq!(
            milliseconds,
            read_static_field(&mut ctx, TIME_UNIT, "MILLISECONDS", TIME_UNIT_DESC)
        );

        let watch = method(&mut ctx, "createUnstarted", WITH_TICKER, &[ticker]);
        assert_eq!(
            method(&mut ctx, "elapsed", ELAPSED, &[watch, milliseconds]),
            V::J(0)
        );
        assert_eq!(ctx.rt.get_field(ticker, "reads").i(), 0);
        method(&mut ctx, "stop", WATCH_RETURN, &[watch]);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/IllegalStateException")
        );
        assert_eq!(ctx.rt.get_field(ticker, "reads"), V::I(1));

        assert_eq!(method(&mut ctx, "start", WATCH_RETURN, &[watch]), watch);
        method(&mut ctx, "start", WATCH_RETURN, &[watch]);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/IllegalStateException")
        );
        assert_eq!(ctx.rt.get_field(ticker, "reads"), V::I(2));
        ctx.rt.put_field(ticker, "tick", V::J(i64::MIN + 1_000_000));
        assert_eq!(
            method(&mut ctx, "elapsed", ELAPSED, &[watch, milliseconds]),
            V::J(1)
        );
        assert_eq!(method(&mut ctx, "stop", WATCH_RETURN, &[watch]), watch);
        assert_eq!(
            method(&mut ctx, "elapsed", ELAPSED, &[watch, nanos]),
            V::J(1_500_000)
        );
        assert_eq!(ctx.rt.get_field(ticker, "reads"), V::I(4));

        // A null target still reads the ticker when running. A subsequent stop
        // may observe a negative duration from a custom ticker that goes backward.
        ctx.rt.put_field(ticker, "tick", V::J(10_000_000));
        assert_eq!(method(&mut ctx, "start", WATCH_RETURN, &[watch]), watch);
        ctx.rt.put_field(ticker, "tick", V::J(7_250_000));
        method(&mut ctx, "elapsed", ELAPSED, &[watch, V::NULL]);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/NullPointerException")
        );
        assert_eq!(ctx.rt.get_field(ticker, "reads"), V::I(6));
        assert_eq!(method(&mut ctx, "stop", WATCH_RETURN, &[watch]), watch);
        assert_eq!(
            method(&mut ctx, "elapsed", ELAPSED, &[watch, nanos]),
            V::J(-1_250_000)
        );
        assert_eq!(
            method(&mut ctx, "elapsed", ELAPSED, &[watch, milliseconds]),
            V::J(-1)
        );

        let failure = ctx.rt.new_instance("test/TickerFailure", 0);
        ctx.rt.put_field(ticker, "failure", failure);
        method(&mut ctx, "stop", WATCH_RETURN, &[watch]);
        assert_eq!(ctx.rt.take_exception(), Some(failure));
        method(&mut ctx, "start", WATCH_RETURN, &[watch]);
        assert_eq!(ctx.rt.take_exception(), Some(failure));
        assert_eq!(method(&mut ctx, "isRunning", "()Z", &[watch]), V::TRUE);
        assert_eq!(method(&mut ctx, "reset", WATCH_RETURN, &[watch]), watch);
        assert_eq!(method(&mut ctx, "isRunning", "()Z", &[watch]), V::FALSE);
        assert_eq!(
            method(&mut ctx, "elapsed", ELAPSED, &[watch, nanos]),
            V::J(0)
        );
        assert_eq!(ctx.rt.get_field(ticker, "reads"), V::I(9));

        method(&mut ctx, "createStarted", WITH_TICKER, &[V::NULL]);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/NullPointerException")
        );
        assert!(call(
            &mut ctx,
            &MemberRef::new(STOPWATCH, "elapsed", "()J"),
            false,
            &[watch]
        )
        .is_none());
        assert!(get_static(ctx.rt, TIME_UNIT, "MILLISECONDS", "I").is_none());
    }

    #[test]
    fn system_stopwatch_accumulates_monotonic_time() {
        let mut rt = Rt::new();
        let mut host = NullHost;
        let linked = LinkedSpace::new();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let watch = method(&mut ctx, "createStarted", WATCH_RETURN, &[]);
        let nanos = read_static_field(&mut ctx, TIME_UNIT, "NANOSECONDS", TIME_UNIT_DESC);
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert!(method(&mut ctx, "elapsed", ELAPSED, &[watch, nanos]).j() > 0);
        assert_eq!(method(&mut ctx, "stop", WATCH_RETURN, &[watch]), watch);
        assert!(method(&mut ctx, "elapsed", ELAPSED, &[watch, nanos]).j() > 0);
        assert!(ctx.rt.pending_exception.is_none());
    }
}
