fn main() {
    // Glibc 2.34 compatibility is handled via global_asm! in src/lib.rs (#416).
    // The previous --defsym=__memcmpeq=memcmp linker flag worked with GNU ld but
    // broke lld (now the default on ubuntu-24.04 CI runners) because lld cannot
    // create --defsym aliases to PLT-resolved shared-library symbols.
}
