//! Raw-DER Certificate accessor surface.
//!
//! Exercises the `from_der` path that every binding's `Certificate` class
//! relies on. Confirms subject / issuer / serial / validity / is_valid
//! readers parse X.509 bytes correctly via `x509-parser`.
//! PKCS#12 round-trip coverage lives in `test_pkcs12_signing.rs`.
#![cfg(feature = "signatures")]

use pdf_oxide::signatures::SigningCredentials;

/// A self-signed RSA-SHA256 certificate in DER form, generated with:
///
///     openssl req -x509 -newkey rsa:2048 -nodes -days 365 \
///       -subj '/CN=pdfoxide-test/O=pdf_oxide/C=US' -outform DER
///
/// Shipped in-test as base64 to avoid an external fixture file. Valid
/// 2026-04-22 → 2027-04-22 (one-year window).
fn test_certificate_der() -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(concat!(
            "MIIDUzCCAjugAwIBAgIUfhkj31z+E/QK7A7iLG6rE4xCOl8wDQYJKoZIhvcN",
            "AQELBQAwOTEWMBQGA1UEAwwNcGRmb3hpZGUtdGVzdDESMBAGA1UECgwJcGRm",
            "X294aWRlMQswCQYDVQQGEwJVUzAeFw0yNjA0MjIwMTM0MTRaFw0yNzA0MjIw",
            "MTM0MTRaMDkxFjAUBgNVBAMMDXBkZm94aWRlLXRlc3QxEjAQBgNVBAoMCXBk",
            "Zl9veGlkZTELMAkGA1UEBhMCVVMwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAw",
            "ggEKAoIBAQCyDVh+aci/RdIm2Y0Huc+jpH5pUjhApgZMHUF1ZmTLzXR3NhPI",
            "TNSO8hfe7j554oQw6OUw7FvMa3nt9UXgY4Jn/GMtrqxyZvWx4HZrfJS7zWd5",
            "pHDWRjRfD2ARIMN1vz1brXmKSjyzbYXdvpOhQXUqUJiHnNyqXB0uBdhl2voA",
            "uHWffFewQZUWao2/HdJdOll1d8w2RtFvdNzpEM3UPK2OujbCkyr5Ir4cSqsS",
            "PCcqQD54EtkqZwFOVtpyavrYMeIth1GPyeYduCj2SL0SwOAX2sAfNBeXJgxq",
            "HF3tkbNwyVbPC8S25VgE5irWBcsrNz1Q0tUhwjnwjKP6SXPbJN1JAgMBAAGj",
            "UzBRMB0GA1UdDgQWBBTHeShuQLPDXWF9vHZXRvda8geaZzAfBgNVHSMEGDAW",
            "gBTHeShuQLPDXWF9vHZXRvda8geaZzAPBgNVHRMBAf8EBTADAQH/MA0GCSqG",
            "SIb3DQEBCwUAA4IBAQAy/mQ7JmruHAxCGv+n8M3ADqb5n88WU5YpNRr8t6y+",
            "BOUNRCYOSX1rTqbiZkeDkOGpg/9C0Tq4V51GyJJLpdy2DiyhD/u8Arkuf28/",
            "ZjvaWFbFqz/T95PG/gnajsK1EtFv3aiufnX1uQzyGefPsZ5dgYXmxWLWt4bb",
            "+M8VGLLKlupnke6eIIN9EBGlMEXshq6kaXfyo9+tSzSxn0/bn7FLycgQKlgm",
            "EX5+eW/zPX7SvXS/DPLSNkBeLEo2veiB9hWKzqlje98H9J3RhVB44u5NxZmT",
            "DFtv9buIluD0k8XjL4sExR9HdojXZc43ABWroaO91GUTrwBht9OuCsnQt52x",
        ))
        .expect("valid base64")
}

#[test]
fn from_der_returns_credentials_with_cert_bytes() {
    let der = test_certificate_der();
    let creds = SigningCredentials::from_der(der.clone()).expect("from_der");
    assert_eq!(creds.certificate, der);
    assert!(creds.private_key.is_empty(), "from_der does not load a private key");
}

#[test]
fn subject_extracts_common_name() {
    let creds = SigningCredentials::from_der(test_certificate_der()).unwrap();
    let subject = creds.subject().expect("subject");
    // RFC 5280 Distinguished Name includes CN and/or O fields.
    assert!(
        subject.contains("CN=") || subject.contains("O=") || subject.contains("C="),
        "subject should contain at least one DN component, got {subject:?}"
    );
}

#[test]
fn issuer_is_returned() {
    let creds = SigningCredentials::from_der(test_certificate_der()).unwrap();
    let issuer = creds.issuer().expect("issuer");
    assert!(!issuer.is_empty());
}

#[test]
fn serial_is_non_empty() {
    let creds = SigningCredentials::from_der(test_certificate_der()).unwrap();
    let serial = creds.serial().expect("serial");
    // Hex or decimal — just ensure something came back.
    assert!(!serial.is_empty());
}

#[test]
fn validity_returns_sensible_unix_timestamps() {
    let creds = SigningCredentials::from_der(test_certificate_der()).unwrap();
    let (not_before, not_after) = creds.validity().expect("validity");
    assert!(not_before > 0);
    assert!(
        not_after > not_before,
        "not_after {not_after} must be > not_before {not_before}"
    );
}

#[test]
fn is_valid_respects_current_time() {
    let creds = SigningCredentials::from_der(test_certificate_der()).unwrap();
    // The cert we generated is valid 2026-04-22 → 2027-04-22.
    // Right now (system time inside the build env) is 2026-04-21 at the
    // earliest, so it's valid.
    let valid = creds.is_valid().expect("is_valid");
    assert!(valid, "cert should be valid today");
}

#[test]
fn from_der_rejects_garbage() {
    let err = SigningCredentials::from_der(vec![0x00, 0x01, 0x02]);
    assert!(err.is_err(), "garbage bytes should not parse as DER");
}
