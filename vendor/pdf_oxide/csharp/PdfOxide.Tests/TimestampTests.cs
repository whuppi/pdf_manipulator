using System;
using PdfOxide.Core;
using PdfOxide.Exceptions;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// Mirror of <c>tests/test_signature_timestamp*</c> — pins that the
    /// RFC 3161 parser lands end-to-end through the FFI:
    /// Parse → DateTimeOffset + Serial + PolicyOid + HashAlgorithm +
    /// MessageImprint + TsaName.
    ///
    /// When the native library is compiled without the <c>signatures</c>
    /// feature, Timestamp.Parse throws <see cref="UnsupportedFeatureException"/>
    /// and the test passes vacuously (same pattern as
    /// <see cref="SignatureTests"/>).
    /// </summary>
    public class TimestampTests
    {
        /// <summary>
        /// Bare TSTInfo bytes (not CMS-wrapped) extracted from the
        /// x509-tsp reference response — openssl-generated, gen_time
        /// 2023-06-07T11:26:26Z.
        /// </summary>
        private static readonly byte[] BareTstInfo = Convert.FromHexString(
            "3081B302010106042A0304013031300D060960864801650304020105000420" +
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD" +
            "020104180F32303233303630373131323632365A300A020101800201F4810164" +
            "0101FF0208314CFCE4E0651827A048A4463044310B30090603550406130255533113" +
            "301106035504080C0A536F6D652D5374617465310D300B060355040A0C04546573" +
            "743111300F06035504030C085465737420545341");

        [Fact]
        public void Parse_BareTstInfo_ExposesFields()
        {
            try
            {
                using var ts = Timestamp.Parse(BareTstInfo);
                Assert.Equal(
                    DateTimeOffset.FromUnixTimeSeconds(1_686_137_186),
                    ts.Time);
                Assert.Equal("04", ts.Serial);
                Assert.Equal("1.2.3.4.1", ts.PolicyOid);
                Assert.Equal(TimestampHashAlgorithm.Sha256, ts.HashAlgorithm);
                Assert.Equal(32, ts.MessageImprint.Length);
                Assert.Equal("CN=Test TSA,O=Test,ST=Some-State,C=US", ts.TsaName);
            }
            catch (UnsupportedFeatureException) { }
        }

        [Fact]
        public void Parse_EmptyThrows()
        {
            Assert.Throws<ArgumentException>(() => Timestamp.Parse(Array.Empty<byte>()));
        }

        [Fact]
        public void Parse_NullThrows()
        {
            Assert.Throws<ArgumentNullException>(() => Timestamp.Parse(null!));
        }

        [Fact]
        public void Parse_GarbageThrowsPdfException()
        {
            Assert.ThrowsAny<PdfException>(
                () => Timestamp.Parse(new byte[] { 0x00, 0x01, 0x02, 0x03 }));
        }

        [Fact]
        public void Verify_CurrentlyUnsupported()
        {
            try
            {
                using var ts = Timestamp.Parse(BareTstInfo);
                // Contract pin: TSA-token signature verification is not
                // yet wired on the Rust side, so Verify() surfaces as
                // UnsupportedFeatureException via ExceptionMapper.
                Assert.Throws<UnsupportedFeatureException>(() => ts.Verify());
            }
            catch (UnsupportedFeatureException) { }
        }

        [Fact]
        public void Dispose_IsIdempotent_AndThrowsAfter()
        {
            try
            {
                var ts = Timestamp.Parse(BareTstInfo);
                ts.Dispose();
                ts.Dispose();
                Assert.Throws<ObjectDisposedException>(() => _ = ts.Serial);
            }
            catch (UnsupportedFeatureException) { }
        }
    }
}
