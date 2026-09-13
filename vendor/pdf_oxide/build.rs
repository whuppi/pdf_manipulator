fn main() {
    // Glibc 2.34 compatibility is handled via global_asm! in src/lib.rs (#416).
    // The previous --defsym=__memcmpeq=memcmp linker flag worked with GNU ld but
    // broke lld (now the default on ubuntu-24.04 CI runners) because lld cannot
    // create --defsym aliases to PLT-resolved shared-library symbols.

    // ── pdf_manipulator patch ──────────────────────────────────
    // Android 16 KB page-size alignment (Google Play API 35+).
    // Cargo doesn't inherit NDK's 16 KB default — emit explicitly.
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("android") {
        println!("cargo:rustc-link-arg=-Wl,-z,max-page-size=16384");
        println!("cargo:rustc-link-arg=-Wl,-z,common-page-size=16384");
    }
    // ── end patch ────────────────────────────────────────────
}
