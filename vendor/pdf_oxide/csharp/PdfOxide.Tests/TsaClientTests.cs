using System;
using PdfOxide.Core;
using PdfOxide.Exceptions;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// Offline unit tests for <see cref="TsaClient"/>. Round-tripping
    /// against a real TSA requires network; that lives in a
    /// sockets-gated integration test so CI stays hermetic.
    ///
    /// When the native library is compiled without the <c>signatures</c>
    /// feature, TsaClient.Create throws <see cref="UnsupportedFeatureException"/>
    /// and the test passes vacuously (same pattern as
    /// <see cref="SignatureTests"/>).
    /// </summary>
    public class TsaClientTests
    {
        [Fact]
        public void Create_NullOptionsThrows()
        {
            Assert.Throws<ArgumentNullException>(() => TsaClient.Create(null!));
        }

        [Fact]
        public void Create_ValidUrl_Succeeds()
        {
            try
            {
                using var client = TsaClient.Create(new TsaClientOptions
                {
                    Url = "https://freetsa.org/tsr",
                });
                Assert.NotNull(client);
            }
            catch (UnsupportedFeatureException) { }
        }

        [Fact]
        public void Dispose_IsIdempotent()
        {
            try
            {
                var client = TsaClient.Create(new TsaClientOptions
                {
                    Url = "https://freetsa.org/tsr",
                });
                client.Dispose();
                client.Dispose(); // must not throw
            }
            catch (UnsupportedFeatureException) { }
        }

        [Fact]
        public void RequestTimestamp_AfterDispose_Throws()
        {
            try
            {
                var client = TsaClient.Create(new TsaClientOptions
                {
                    Url = "https://freetsa.org/tsr",
                });
                client.Dispose();
                Assert.Throws<ObjectDisposedException>(() =>
                    client.RequestTimestamp(new byte[] { 1, 2, 3 }));
            }
            catch (UnsupportedFeatureException) { }
        }

        [Fact]
        public void Options_DefaultsMatchRustCore()
        {
            var opts = new TsaClientOptions { Url = "https://example.test/tsr" };
            Assert.Equal(30, opts.TimeoutSeconds);
            Assert.Equal(TimestampHashAlgorithm.Sha256, opts.HashAlgorithm);
            Assert.True(opts.UseNonce);
            Assert.True(opts.CertReq);
            Assert.Null(opts.Username);
            Assert.Null(opts.Password);
        }
    }
}
