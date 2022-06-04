//! Performs autodetection of the host for the purposes of running
//! Cranelift to generate code to run on the same machine.

#![deny(
    missing_docs,
    trivial_numeric_casts,
    unused_extern_crates,
    unstable_features
)]
#![warn(unused_import_braces)]
#![cfg_attr(feature = "clippy", plugin(clippy(conf_file = "../../clippy.toml")))]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::new_without_default))]
#![cfg_attr(
    feature = "cargo-clippy",
    warn(
        clippy::float_arithmetic,
        clippy::mut_mut,
        clippy::nonminimal_bool,
        clippy::map_unwrap_or,
        clippy::clippy::print_stdout,
        clippy::unicode_not_nfc,
        clippy::use_self
    )
)]

use cranelift_codegen::isa;
use target_lexicon::Triple;

/// Return an `isa` builder configured for the current host
/// machine, or `Err(())` if the host machine is not supported
/// in the current configuration.
pub fn builder() -> Result<isa::Builder, &'static str> {
    builder_with_options(true)
}

/// Return an `isa` builder configured for the current host
/// machine, or `Err(())` if the host machine is not supported
/// in the current configuration.
///
/// Selects the given backend variant specifically; this is
/// useful when more than oen backend exists for a given target
/// (e.g., on x86-64).
pub fn builder_with_options(infer_native_flags: bool) -> Result<isa::Builder, &'static str> {
    use cranelift_codegen::settings::Configurable;
    // A helper to set a feature flag to the given value.
    fn set(isa_builder: &mut isa::Builder, name: &str, detected: bool) {
        isa_builder
            .set(name, if detected { "1" } else { "0" })
            .unwrap();
    }

    let mut isa_builder = isa::lookup(Triple::host()).map_err(|err| match err {
        isa::LookupError::SupportDisabled => "support for architecture disabled at compile time",
        isa::LookupError::Unsupported => "unsupported architecture",
    })?;

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if !std::is_x86_feature_detected!("sse2") {
            return Err("x86 support requires SSE2");
        }

        if !infer_native_flags {
            return Ok(isa_builder);
        }

        set(
            &mut isa_builder,
            "has_sse3",
            std::is_x86_feature_detected!("sse3"),
        );
        set(
            &mut isa_builder,
            "has_ssse3",
            std::is_x86_feature_detected!("ssse3"),
        );
        set(
            &mut isa_builder,
            "has_sse41",
            std::is_x86_feature_detected!("sse4.1"),
        );
        set(
            &mut isa_builder,
            "has_sse42",
            std::is_x86_feature_detected!("sse4.2"),
        );
        set(
            &mut isa_builder,
            "has_popcnt",
            std::is_x86_feature_detected!("popcnt"),
        );
        set(
            &mut isa_builder,
            "has_avx",
            std::is_x86_feature_detected!("avx"),
        );
        set(
            &mut isa_builder,
            "has_avx2",
            std::is_x86_feature_detected!("avx2"),
        );
        set(
            &mut isa_builder,
            "has_bmi1",
            std::is_x86_feature_detected!("bmi1"),
        );
        set(
            &mut isa_builder,
            "has_bmi2",
            std::is_x86_feature_detected!("bmi2"),
        );
        set(
            &mut isa_builder,
            "has_avx512bitalg",
            std::is_x86_feature_detected!("avx512bitalg"),
        );
        set(
            &mut isa_builder,
            "has_avx512dq",
            std::is_x86_feature_detected!("avx512dq"),
        );
        set(
            &mut isa_builder,
            "has_avx512f",
            std::is_x86_feature_detected!("avx512f"),
        );
        set(
            &mut isa_builder,
            "has_avx512vl",
            std::is_x86_feature_detected!("avx512vl"),
        );
        set(
            &mut isa_builder,
            "has_avx512vbmi",
            std::is_x86_feature_detected!("avx512vbmi"),
        );
        set(
            &mut isa_builder,
            "has_lzcnt",
            std::is_x86_feature_detected!("lzcnt"),
        );
    }

    #[cfg(target_arch = "aarch64")]
    {
        if !infer_native_flags {
            return Ok(isa_builder);
        }

        set(
            &mut isa_builder,
            "has_lse",
            std::is_aarch64_feature_detected!("lse"),
        );
    }

    // There is no is_s390x_feature_detected macro yet, so for now
    // we use getauxval from the libc crate directly.
    #[cfg(all(target_arch = "s390x", target_os = "linux"))]
    {
        if !infer_native_flags {
            return Ok(isa_builder);
        }

        let v = unsafe { libc::getauxval(libc::AT_HWCAP) };
        const HWCAP_S390X_VXRS_EXT2: libc::c_ulong = 32768;
        let vxrs_ext2 = (v & HWCAP_S390X_VXRS_EXT2) != 0;
        set(&mut isa_builder, "has_vxrs_ext2", vxrs_ext2);
        // There is no separate HWCAP bit for mie2, so assume
        // that any machine with vxrs_ext2 also has mie2.
        set(&mut isa_builder, "has_mie2", vxrs_ext2);
    }

    // squelch warnings about unused mut/variables on some platforms.
    drop(&mut isa_builder);
    drop(infer_native_flags);

    Ok(isa_builder)
}

#[cfg(test)]
mod tests {
    use super::builder;
    use cranelift_codegen::isa::CallConv;
    use cranelift_codegen::settings;

    #[test]
    fn test() {
        if let Ok(isa_builder) = builder() {
            let flag_builder = settings::builder();
            let isa = isa_builder
                .finish(settings::Flags::new(flag_builder))
                .unwrap();

            if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
                assert_eq!(isa.default_call_conv(), CallConv::AppleAarch64);
            } else if cfg!(any(unix, target_os = "nebulet")) {
                assert_eq!(isa.default_call_conv(), CallConv::SystemV);
            } else if cfg!(windows) {
                assert_eq!(isa.default_call_conv(), CallConv::WindowsFastcall);
            }

            if cfg!(target_pointer_width = "64") {
                assert_eq!(isa.pointer_bits(), 64);
            } else if cfg!(target_pointer_width = "32") {
                assert_eq!(isa.pointer_bits(), 32);
            } else if cfg!(target_pointer_width = "16") {
                assert_eq!(isa.pointer_bits(), 16);
            }
        }
    }
}

/// Version number of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
