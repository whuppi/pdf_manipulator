"""Python Certificate class tests.

Mirrors coverage in csharp/PdfOxide.Tests/CertificateTests.cs and
go/certificate_test.go so every binding stays in lockstep. The fixture
is the same self-signed RSA-SHA256 DER used by the Rust suite in
tests/test_certificate_accessors.rs.
"""

import base64
from datetime import datetime, timezone

import pytest


pdf_oxide = pytest.importorskip(
    "pdf_oxide.pdf_oxide", reason="pdf_oxide native module not importable"
)
Certificate = getattr(pdf_oxide, "Certificate", None)

# Self-signed RSA-SHA256 cert valid 2026-04-22 → 2027-04-22. Same fixture
# as the C# / Go / Rust test suites — keeping it in sync matters because
# the Validity assertions lock to this specific window.
_TEST_CERT_DER = base64.b64decode(
    "MIIDUzCCAjugAwIBAgIUfhkj31z+E/QK7A7iLG6rE4xCOl8wDQYJKoZIhvcNAQEL"
    "BQAwOTEWMBQGA1UEAwwNcGRmb3hpZGUtdGVzdDESMBAGA1UECgwJcGRmX294aWRl"
    "MQswCQYDVQQGEwJVUzAeFw0yNjA0MjIwMTM0MTRaFw0yNzA0MjIwMTM0MTRaMDkx"
    "FjAUBgNVBAMMDXBkZm94aWRlLXRlc3QxEjAQBgNVBAoMCXBkZl9veGlkZTELMAkG"
    "A1UEBhMCVVMwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCyDVh+aci/"
    "RdIm2Y0Huc+jpH5pUjhApgZMHUF1ZmTLzXR3NhPITNSO8hfe7j554oQw6OUw7FvM"
    "a3nt9UXgY4Jn/GMtrqxyZvWx4HZrfJS7zWd5pHDWRjRfD2ARIMN1vz1brXmKSjyz"
    "bYXdvpOhQXUqUJiHnNyqXB0uBdhl2voAuHWffFewQZUWao2/HdJdOll1d8w2RtFv"
    "dNzpEM3UPK2OujbCkyr5Ir4cSqsSPCcqQD54EtkqZwFOVtpyavrYMeIth1GPyeYd"
    "uCj2SL0SwOAX2sAfNBeXJgxqHF3tkbNwyVbPC8S25VgE5irWBcsrNz1Q0tUhwjnw"
    "jKP6SXPbJN1JAgMBAAGjUzBRMB0GA1UdDgQWBBTHeShuQLPDXWF9vHZXRvda8gea"
    "ZzAfBgNVHSMEGDAWgBTHeShuQLPDXWF9vHZXRvda8geaZzAPBgNVHRMBAf8EBTAD"
    "AQH/MA0GCSqGSIb3DQEBCwUAA4IBAQAy/mQ7JmruHAxCGv+n8M3ADqb5n88WU5Yp"
    "NRr8t6y+BOUNRCYOSX1rTqbiZkeDkOGpg/9C0Tq4V51GyJJLpdy2DiyhD/u8Arku"
    "f28/ZjvaWFbFqz/T95PG/gnajsK1EtFv3aiufnX1uQzyGefPsZ5dgYXmxWLWt4bb"
    "+M8VGLLKlupnke6eIIN9EBGlMEXshq6kaXfyo9+tSzSxn0/bn7FLycgQKlgmEX5+"
    "eW/zPX7SvXS/DPLSNkBeLEo2veiB9hWKzqlje98H9J3RhVB44u5NxZmTDFtv9buI"
    "luD0k8XjL4sExR9HdojXZc43ABWroaO91GUTrwBht9OuCsnQt52x"
)


pytestmark = pytest.mark.skipif(
    Certificate is None, reason="pdf_oxide built without --features signatures"
)


def _load():
    return Certificate.load(_TEST_CERT_DER)


def test_load_from_der_succeeds():
    cert = _load()
    assert cert is not None


def test_load_empty_raises():
    with pytest.raises(ValueError):
        Certificate.load(b"")


def test_load_garbage_raises():
    with pytest.raises(ValueError):
        Certificate.load(b"not a cert" * 100)


def test_subject_contains_dn():
    cert = _load()
    assert "pdfoxide-test" in cert.subject


def test_issuer_nonempty():
    cert = _load()
    assert cert.issuer


def test_serial_is_hex():
    cert = _load()
    serial = cert.serial.replace(":", "").lower()
    assert serial, "serial must be non-empty"
    assert all(c in "0123456789abcdef" for c in serial), serial


def test_validity_tuple_shape():
    cert = _load()
    nb, na = cert.validity
    assert isinstance(nb, int) and isinstance(na, int)
    assert nb < na


def test_validity_matches_fixture_window():
    cert = _load()
    nb, na = cert.validity
    # 2026-04-22 → 2027-04-22, with a day of slack either side.
    want_start = int(datetime(2026, 4, 21, tzinfo=timezone.utc).timestamp())
    want_end = int(datetime(2027, 4, 23, tzinfo=timezone.utc).timestamp())
    assert nb >= want_start, (nb, want_start)
    assert na <= want_end, (na, want_end)


def test_is_valid_true_in_window():
    cert = _load()
    # Fixture is valid 2026-04-22 → 2027-04-22; test is written on
    # 2026-04-22. If this fails, check the system clock.
    assert cert.is_valid is True


def test_repr_has_subject_and_serial():
    cert = _load()
    r = repr(cert)
    assert "Certificate(" in r
    assert "subject=" in r
    assert "serial=" in r
