// RFC 3161 Timestamp parsing + TsaClient construction
//
// Parses a pre-encoded bare TSTInfo blob and verifies its fields.
// Also constructs a TsaClient (requires tsa-client feature).
//
// Requires --features signatures (Timestamp) or --features signatures,tsa-client (both)
//   cargo run --example showcase_rfc3161_timestamp --features signatures
//   cargo run --example showcase_rfc3161_timestamp --features signatures,tsa-client

#[cfg(not(feature = "signatures"))]
fn main() {
    println!("SKIP: build with --features signatures to run this example.");
}

#[cfg(feature = "signatures")]
fn main() -> pdf_oxide::error::Result<()> {
    use pdf_oxide::signatures::{HashAlgorithm, Timestamp};

    // ── 1. Timestamp parsing ─────────────────────────────────────────────────
    println!("Parsing RFC 3161 timestamp...");

    let bare_tst_info = hex_decode(concat!(
        "3081B302010106042A0304013031300D060960864801650304020105000420",
        "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD",
        "020104180F32303233303630373131323632365A300A020101800201F4810164",
        "0101FF0208314CFCE4E0651827A048A4463044310B30090603550406130255533113",
        "301106035504080C0A536F6D652D5374617465310D300B060355040A0C04546573",
        "743111300F06035504030C085465737420545341",
    ));

    let ts = Timestamp::from_der(&bare_tst_info)?;
    println!("  Time (epoch): {}", ts.time());
    println!("  Serial: {}  Policy OID: {}", ts.serial(), ts.policy_oid());
    println!("  TSA name: {}", ts.tsa_name());
    assert_eq!(ts.serial(), "04", "unexpected serial");
    assert_eq!(ts.policy_oid(), "1.2.3.4.1", "unexpected policy OID");
    println!("  Timestamp fields verified.");

    let _ = HashAlgorithm::Sha256; // suppress unused import

    // ── 2. TsaClient construction (offline) — requires tsa-client feature ────
    #[cfg(feature = "tsa-client")]
    {
        use pdf_oxide::signatures::{TsaClient, TsaClientConfig};
        println!("Constructing TsaClient (offline, no network call)...");
        let cfg = TsaClientConfig::new("https://freetsa.org/tsr");
        let _client = TsaClient::new(cfg);
        println!("  TsaClient created (no network call).");
    }
    #[cfg(not(feature = "tsa-client"))]
    {
        println!("TsaClient skipped (build with --features tsa-client to enable).");
    }

    Ok(())
}

#[cfg(feature = "signatures")]
fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}
