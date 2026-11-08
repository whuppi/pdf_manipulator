using System;
using System.Linq;
using PdfOxide.Core;
using PdfOxide.Exceptions;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// Tests for the Certificate public class. Matches the Rust-level
    /// coverage in <c>tests/test_certificate_accessors.rs</c>.
    ///
    /// When the native library is compiled without the <c>signatures</c>
    /// feature, Certificate.Load throws <see cref="UnsupportedFeatureException"/>
    /// and each test passes vacuously (same pattern as
    /// <see cref="SignatureTests"/>).
    /// </summary>
    public class CertificateTests
    {
        /// <summary>
        /// Self-signed RSA-SHA256 cert valid 2026-04-22 → 2027-04-22,
        /// generated with:
        ///   openssl req -x509 -newkey rsa:2048 -nodes -days 365 \
        ///     -subj '/CN=pdfoxide-test/O=pdf_oxide/C=US' -outform DER
        /// </summary>
        private static byte[] TestCertificateDer() => Convert.FromBase64String(
            "MIIDUzCCAjugAwIBAgIUfhkj31z+E/QK7A7iLG6rE4xCOl8wDQYJKoZIhvcNAQEL" +
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
            "luD0k8XjL4sExR9HdojXZc43ABWroaO91GUTrwBht9OuCsnQt52x");

        [Fact]
        public void Load_Der_Succeeds()
        {
            try
            {
                using var cert = Certificate.Load(TestCertificateDer());
                Assert.NotNull(cert);
            }
            catch (UnsupportedFeatureException) { }
        }

        [Fact]
        public void Subject_ContainsDnFragment()
        {
            try
            {
                using var cert = Certificate.Load(TestCertificateDer());
                var subject = cert.Subject;
                Assert.False(string.IsNullOrEmpty(subject));
                Assert.Contains("CN=", subject, StringComparison.OrdinalIgnoreCase);
            }
            catch (UnsupportedFeatureException) { }
        }

        [Fact]
        public void Issuer_IsNonEmpty()
        {
            try
            {
                using var cert = Certificate.Load(TestCertificateDer());
                Assert.False(string.IsNullOrEmpty(cert.Issuer));
            }
            catch (UnsupportedFeatureException) { }
        }

        [Fact]
        public void Serial_IsHex()
        {
            try
            {
                using var cert = Certificate.Load(TestCertificateDer());
                var serial = cert.Serial;
                Assert.False(string.IsNullOrEmpty(serial));
                Assert.True(serial.All(c => "0123456789abcdefABCDEF".IndexOf(c) >= 0),
                    $"expected hex, got '{serial}'");
            }
            catch (UnsupportedFeatureException) { }
        }

        [Fact]
        public void Validity_Window_IsCoherent()
        {
            try
            {
                using var cert = Certificate.Load(TestCertificateDer());
                var (nb, na) = cert.Validity;
                Assert.True(na > nb, "not_after must be after not_before");
                var span = na - nb;
                Assert.True(span.TotalDays > 300, $"validity span was {span.TotalDays:0} days");
            }
            catch (UnsupportedFeatureException) { }
        }

        [Fact]
        public void Validity_MatchesFixtureWindow()
        {
            try
            {
                using var cert = Certificate.Load(TestCertificateDer());
                var (nb, na) = cert.Validity;
                Assert.Equal(new DateTime(2026, 4, 22), nb.Date);
                Assert.Equal(new DateTime(2027, 4, 22), na.Date);
                Assert.True(na > nb, "not_after must be after not_before");
            }
            catch (UnsupportedFeatureException) { }
        }

        [Fact]
        public void Load_Empty_Throws()
        {
            Assert.Throws<ArgumentException>(() => Certificate.Load(Array.Empty<byte>()));
        }

        [Fact]
        public void Load_Null_Throws()
        {
            Assert.Throws<ArgumentNullException>(() => Certificate.Load(null!));
        }

        [Fact]
        public void Load_Garbage_ThrowsPdfException()
        {
            Assert.ThrowsAny<PdfException>(
                () => Certificate.Load(new byte[] { 0x00, 0x01, 0x02, 0x03 }));
        }

        [Fact]
        public void Dispose_IsIdempotent_And_Throws_After()
        {
            try
            {
                var cert = Certificate.Load(TestCertificateDer());
                cert.Dispose();
                cert.Dispose();
                Assert.Throws<ObjectDisposedException>(() => _ = cert.Subject);
            }
            catch (UnsupportedFeatureException) { }
        }
    }
}
