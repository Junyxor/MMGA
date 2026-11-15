//! `mmga-runtime`: Zero-JVM Object Model, Write-Barrier Generational GC,
//! Runtime-Linked Class Space, Native JDK Stdlib, and Tier-0/Tier-1 Hybrid Execution Engine.

pub mod fxhash;
pub mod heap;
pub mod linked;
pub mod stdlib;
pub mod value;
pub mod vm;

pub use fxhash::{fnv1a_64, FxHashMap, FxHashSet, FxHasher};
pub use heap::{ClosureData, ClosureTarget, Data, Object, Rt, RuntimeTelemetry};
pub use linked::{
    ExecCtx, HostApi, LinkedSpace, NativeClassDef, NativeLambdaFn, NativeMethodFn,
    NativeModuleRegistry, NullHost, ResolvedMethod,
};
pub use stdlib::{box_primitive, stdlib_call, stdlib_get_static, unbox_primitive};
pub use value::{OperandStack, V};
pub use vm::{
    alloc_new_object, default_value_for_desc, ensure_class_init, execute_invokedynamic,
    invoke_closure, invoke_member, invoke_resolved, read_instance_field, read_static_field,
    write_instance_field, write_static_field,
};

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::ClassFile;
    use std::collections::BTreeMap;
    use std::process::Command;

    #[test]
    fn executes_real_javac_compiled_lambdas_records_streams_and_mixins() {
        if Command::new("javac").arg("-version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mmga-rt-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("org/spongepowered/asm/mixin/injection/callback"))
            .unwrap();
        std::fs::create_dir_all(dir.join("com/llamalad7/mixinextras/injector")).unwrap();
        std::fs::create_dir_all(dir.join("demo")).unwrap();

        let files = [
            (
                "org/spongepowered/asm/mixin/Mixin.java",
                r#"package org.spongepowered.asm.mixin;
                import java.lang.annotation.*;
                @Retention(RetentionPolicy.CLASS)
                public @interface Mixin { Class<?>[] value() default {}; int priority() default 1000; }"#,
            ),
            (
                "org/spongepowered/asm/mixin/injection/At.java",
                r#"package org.spongepowered.asm.mixin.injection;
                import java.lang.annotation.*;
                @Retention(RetentionPolicy.CLASS)
                public @interface At { String value(); String target() default ""; String shift() default "BEFORE"; int ordinal() default -1; }"#,
            ),
            (
                "org/spongepowered/asm/mixin/injection/Inject.java",
                r#"package org.spongepowered.asm.mixin.injection;
                import java.lang.annotation.*;
                @Retention(RetentionPolicy.CLASS)
                public @interface Inject { String[] method(); At[] at(); boolean cancellable() default false; }"#,
            ),
            (
                "org/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable.java",
                r#"package org.spongepowered.asm.mixin.injection.callback;
                public class CallbackInfoReturnable<R> {
                    public void setReturnValue(R val) {}
                    public R getReturnValue() { return null; }
                }"#,
            ),
            (
                "com/llamalad7/mixinextras/injector/ModifyReturnValue.java",
                r#"package com.llamalad7.mixinextras.injector;
                import java.lang.annotation.*;
                import org.spongepowered.asm.mixin.injection.At;
                @Retention(RetentionPolicy.CLASS)
                public @interface ModifyReturnValue { String[] method(); At[] at(); }"#,
            ),
            (
                "demo/Machine.java",
                r#"package demo;
                import java.util.*;
                import java.util.stream.*;

                public class Machine {
                    public record Gear(String id, int teeth) {}

                    private int bonus = 2;

                    public int computeRpm(int base) {
                        List<Gear> gears = List.of(new Gear("small", 8), new Gear("large", 16), new Gear("tiny", 4));
                        int sum = gears.stream()
                            .filter(g -> g.teeth() >= 8)
                            .map(g -> g.teeth() * bonus)
                            .reduce(0, Integer::sum);
                        return base + sum;
                    }

                    public String describe(int rpm) {
                        Gear g = new Gear("shaft", rpm);
                        return "Status:" + g.toString();
                    }
                }"#,
            ),
            (
                "demo/MachineMixin.java",
                r#"package demo;
                import org.spongepowered.asm.mixin.Mixin;
                import org.spongepowered.asm.mixin.injection.At;
                import com.llamalad7.mixinextras.injector.ModifyReturnValue;

                @Mixin(Machine.class)
                public abstract class MachineMixin {
                    private int mixinMultiplier = 3;

                    @ModifyReturnValue(method = "computeRpm", at = @At("RETURN"))
                    private int boostRpm(int original) {
                        return original * mixinMultiplier;
                    }
                }"#,
            ),
        ];

        let mut java_paths = Vec::new();
        for (rel, src) in files {
            let p = dir.join(rel);
            std::fs::write(&p, src).unwrap();
            java_paths.push(p);
        }
        let status = Command::new("javac")
            .arg("-d")
            .arg(&dir)
            .args(&java_paths)
            .status()
            .unwrap();
        assert!(status.success(), "javac failed");

        let mut classes = BTreeMap::new();
        for cls_rel in [
            "demo/Machine.class",
            "demo/Machine$Gear.class",
            "demo/MachineMixin.class",
        ] {
            let bytes = std::fs::read(dir.join(cls_rel)).unwrap();
            let cf = ClassFile::parse(&bytes).unwrap();
            classes.insert(cf.name.clone(), cf);
        }
        let _ = std::fs::remove_dir_all(&dir);

        let mut linked = LinkedSpace::new();
        linked.add_bytecode_classes(classes);
        let mut rt = Rt::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };

        let machine = alloc_new_object(&mut ctx, "demo/Machine");
        let _ = invoke_member(
            &mut ctx,
            mmga_classfile::op::INVOKESPECIAL,
            &mmga_classfile::MemberRef::new("demo/Machine", "<init>", "()V"),
            &[machine],
        );

        // `gears` with teeth >= 8: 8*2 + 16*2 = 48. `base` = 10 -> 58.
        // `MachineMixin` initializes `mixinMultiplier = 3` in `<init>`, and `@ModifyReturnValue` multiplies 58 * 3 = 174!
        let rpm = invoke_member(
            &mut ctx,
            mmga_classfile::op::INVOKEVIRTUAL,
            &mmga_classfile::MemberRef::new("demo/Machine", "computeRpm", "(I)I"),
            &[machine, V::I(10)],
        );
        assert_eq!(rpm.i(), 174);

        let desc_v = invoke_member(
            &mut ctx,
            mmga_classfile::op::INVOKEVIRTUAL,
            &mmga_classfile::MemberRef::new(
                "demo/Machine",
                "describe",
                "(I)Ljava/lang/String;",
            ),
            &[machine, rpm],
        );
        assert_eq!(
            ctx.rt.str_of(desc_v),
            Some("Status:Gear[id=shaft, teeth=174]")
        );
    }
}
