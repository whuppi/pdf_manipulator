//go:build cgo

package pdfoxide

// Certificate inspect tests. Mirrors coverage in
// tests/test_certificate_accessors.rs + csharp/PdfOxide.Tests/CertificateTests.cs
// so the Go surface stays in lockstep with the other bindings.

import (
	"encoding/base64"
	"strings"
	"testing"
	"time"
)

// testCertificateDer is the same self-signed RSA-SHA256 cert fixture used
// by the Rust + C# suites — valid 2026-04-22 → 2027-04-22, generated with:
//
//	openssl req -x509 -newkey rsa:2048 -nodes -days 365 \
//	  -subj '/CN=pdfoxide-test/O=pdf_oxide/C=US' -outform DER
func testCertificateDer(t *testing.T) []byte {
	t.Helper()
	const b64 = "MIIDUzCCAjugAwIBAgIUfhkj31z+E/QK7A7iLG6rE4xCOl8wDQYJKoZIhvcNAQEL" +
		"BQAwOTEWMBQGA1UEAwwNcGRmb3hpZGUtdGVzdDESMBAGA1UECgwJcGRmX294aWRl" +
		"MQswCQYDVQQGEwJVUzAeFw0yNjA0MjIwMTM0MTRaFw0yNzA0MjIwMTM0MTRaMDkx" +
		"FjAUBgNVBAMMDXBkZm94aWRlLXRlc3QxEjAQBgNVBAoMCXBkZl9veGlkZTELMAkG" +
		"A1UEBhMCVVMwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCyDVh+aci/" +
		"RdIm2Y0Huc+jpH5pUjhApgZMHUF1ZmTLzXR3NhPITNSO8hfe7j554oQw6OUw7FvM" +
		"a3nt9UXgY4Jn/GMtrqxyZvWx4HZrfJS7zWd5pHDWRjRfD2ARIMN1vz1brXmKSjyz" +
		"bYXdvpOhQXUqUJiHnNyqXB0uBdhl2voAuHWffFewQZUWao2/HdJdOll1d8w2RtFv" +
		"dNzpEM3UPK2OujbCkyr5Ir4cSqsSPCcqQD54EtkqZwFOVtpyavrYMeIth1GPyeYd" +
		"uCj2SL0SwOAX2sAfNBeXJgxqHF3tkbNwyVbPC8S25VgE5irWBcsrNz1Q0tUhwjnw" +
		"jKP6SXPbJN1JAgMBAAGjUzBRMB0GA1UdDgQWBBTHeShuQLPDXWF9vHZXRvda8gea" +
		"ZzAfBgNVHSMEGDAWgBTHeShuQLPDXWF9vHZXRvda8geaZzAPBgNVHRMBAf8EBTAD" +
		"AQH/MA0GCSqGSIb3DQEBCwUAA4IBAQAy/mQ7JmruHAxCGv+n8M3ADqb5n88WU5Yp" +
		"NRr8t6y+BOUNRCYOSX1rTqbiZkeDkOGpg/9C0Tq4V51GyJJLpdy2DiyhD/u8Arku" +
		"f28/ZjvaWFbFqz/T95PG/gnajsK1EtFv3aiufnX1uQzyGefPsZ5dgYXmxWLWt4bb" +
		"+M8VGLLKlupnke6eIIN9EBGlMEXshq6kaXfyo9+tSzSxn0/bn7FLycgQKlgmEX5+" +
		"eW/zPX7SvXS/DPLSNkBeLEo2veiB9hWKzqlje98H9J3RhVB44u5NxZmTDFtv9buI" +
		"luD0k8XjL4sExR9HdojXZc43ABWroaO91GUTrwBht9OuCsnQt52x"
	der, err := base64.StdEncoding.DecodeString(b64)
	if err != nil {
		t.Fatalf("decode fixture: %v", err)
	}
	return der
}

func loadTestCertificate(t *testing.T) *Certificate {
	t.Helper()
	cert, err := LoadCertificate(testCertificateDer(t), "")
	if err != nil {
		t.Skipf("LoadCertificate unavailable in this build: %v", err)
	}
	t.Cleanup(cert.Close)
	return cert
}

func TestCertificate_Load_Succeeds(t *testing.T) {
	cert := loadTestCertificate(t)
	if cert == nil || cert.handle == nil {
		t.Fatal("expected non-nil certificate handle")
	}
}

func TestCertificate_Subject_ContainsDn(t *testing.T) {
	cert := loadTestCertificate(t)
	subject, err := cert.Subject()
	if err != nil {
		t.Fatalf("Subject: %v", err)
	}
	// CN=pdfoxide-test should appear regardless of DN string formatting.
	if !strings.Contains(subject, "pdfoxide-test") {
		t.Errorf("Subject %q does not contain 'pdfoxide-test'", subject)
	}
}

func TestCertificate_Issuer_NonEmpty(t *testing.T) {
	cert := loadTestCertificate(t)
	issuer, err := cert.Issuer()
	if err != nil {
		t.Fatalf("Issuer: %v", err)
	}
	if issuer == "" {
		t.Error("Issuer returned empty string")
	}
}

func TestCertificate_Serial_IsHex(t *testing.T) {
	cert := loadTestCertificate(t)
	serial, err := cert.Serial()
	if err != nil {
		t.Fatalf("Serial: %v", err)
	}
	if serial == "" {
		t.Fatal("Serial returned empty string")
	}
	// Accept hex with optional separators; just assert it's non-empty and
	// every char is hex-digit-or-separator. The Rust side formats serial
	// as colon-separated upper-hex, but we don't want to lock the format.
	for _, c := range strings.ReplaceAll(strings.ToLower(serial), ":", "") {
		if (c < '0' || c > '9') && (c < 'a' || c > 'f') {
			t.Errorf("Serial %q contains non-hex character %q", serial, c)
			break
		}
	}
}

func TestCertificate_Validity_WindowCoherent(t *testing.T) {
	cert := loadTestCertificate(t)
	nb, na, err := cert.Validity()
	if err != nil {
		t.Fatalf("Validity: %v", err)
	}
	if !nb.Before(na) {
		t.Errorf("Validity window not ordered: notBefore=%v notAfter=%v", nb, na)
	}
	// Fixture: 2026-04-22 → 2027-04-22. Allow a day of slack on each side
	// to tolerate TZ differences between the cert and the Go side.
	wantStart := time.Date(2026, 4, 21, 0, 0, 0, 0, time.UTC)
	wantEnd := time.Date(2027, 4, 23, 0, 0, 0, 0, time.UTC)
	if nb.Before(wantStart) || na.After(wantEnd) {
		t.Errorf("Validity outside expected fixture window: notBefore=%v notAfter=%v", nb, na)
	}
}

// TestCertificate_IsValid_MatchesWindow asserts that IsValid() agrees with
// "now is within [notBefore, notAfter]" as reported by Validity(). This keeps
// the test self-consistent regardless of wall-clock — after the fixture's
// notAfter elapses IsValid() flips to false and the test still passes because
// the window check also reports out-of-window. The C# suite avoids a raw
// `IsValid == true` assertion for the same reason (see CertificateTests.cs
// Validity_MatchesFixtureWindow).
func TestCertificate_IsValid_MatchesWindow(t *testing.T) {
	cert := loadTestCertificate(t)
	nb, na, err := cert.Validity()
	if err != nil {
		t.Fatalf("Validity: %v", err)
	}
	ok, err := cert.IsValid()
	if err != nil {
		t.Fatalf("IsValid: %v", err)
	}
	now := time.Now().UTC()
	expected := !now.Before(nb) && !now.After(na)
	if ok != expected {
		t.Errorf("IsValid()=%v but now=%v within [%v, %v]=%v", ok, now, nb, na, expected)
	}
}

func TestCertificate_AfterClose_Errors(t *testing.T) {
	cert, err := LoadCertificate(testCertificateDer(t), "")
	if err != nil {
		t.Skipf("LoadCertificate unavailable: %v", err)
	}
	cert.Close()
	if _, err := cert.Subject(); err == nil {
		t.Error("Subject after Close(): expected error, got nil")
	}
	// Close() should be idempotent.
	cert.Close()
}
