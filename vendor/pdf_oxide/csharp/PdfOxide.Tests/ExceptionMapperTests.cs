using PdfOxide.Exceptions;
using PdfOxide.Internal;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// Verifies that <see cref="ExceptionMapper.CreateException"/> maps every FFI
    /// error code to the <see cref="PdfException"/> subclass that reflects the
    /// Rust-side meaning.
    ///
    /// Error codes are defined in <c>src/ffi.rs</c> at lines 48-56:
    ///   0 = SUCCESS
    ///   1 = INVALID_ARG
    ///   2 = IO
    ///   3 = PARSE
    ///   4 = EXTRACTION
    ///   5 = INTERNAL
    ///   6 = INVALID_PAGE
    ///   7 = SEARCH
    ///   8 = UNSUPPORTED
    ///
    /// Regression: u/gevorgter (Reddit, 2026-04-21) reported that calling
    /// <c>doc.RenderPage(0, 0)</c> on Windows 11 threw <see cref="SignatureException"/>
    /// because the mapper labelled FFI code 8 (unsupported) as "signature". Every code
    /// below 8 was similarly shifted by one.
    /// </summary>
    public class ExceptionMapperTests
    {
        [Fact]
        public void Success_ThrowsArgumentOutOfRange()
        {
            Assert.Throws<System.ArgumentOutOfRangeException>(
                () => ExceptionMapper.CreateException(0));
        }

        [Fact]
        public void Code1_Is_InvalidParameter_ForInvalidArg()
        {
            var ex = ExceptionMapper.CreateException(1);
            Assert.IsType<InvalidParameter>(ex);
        }

        [Fact]
        public void Code2_Is_IoException()
        {
            var ex = ExceptionMapper.CreateException(2);
            Assert.IsType<IoException>(ex);
        }

        [Fact]
        public void Code3_Is_ParseException()
        {
            var ex = ExceptionMapper.CreateException(3);
            Assert.IsType<ParseException>(ex);
        }

        [Fact]
        public void Code4_Is_ParseException_Extraction()
        {
            // Extraction failures are reported as parse errors — no dedicated type today.
            var ex = ExceptionMapper.CreateException(4);
            Assert.IsType<ParseException>(ex);
        }

        [Fact]
        public void Code5_Is_InternalError()
        {
            var ex = ExceptionMapper.CreateException(5);
            Assert.IsType<InternalError>(ex);
        }

        [Fact]
        public void Code6_Is_InvalidParameter_ForInvalidPageIndex()
        {
            var ex = ExceptionMapper.CreateException(6);
            Assert.IsType<InvalidParameter>(ex);
        }

        [Fact]
        public void Code7_Is_SearchException()
        {
            var ex = ExceptionMapper.CreateException(7);
            Assert.IsType<SearchException>(ex);
        }

        /// <summary>
        /// Regression test for u/gevorgter — render failures returning FFI code 8
        /// (unsupported) must NOT surface as SignatureException.
        /// </summary>
        [Fact]
        public void Code8_Is_UnsupportedFeatureException_NotSignatureException()
        {
            var ex = ExceptionMapper.CreateException(8);
            Assert.IsNotType<SignatureException>(ex);
            Assert.IsType<UnsupportedFeatureException>(ex);
        }

        [Fact]
        public void UnknownCode_IsUnknownError()
        {
            var ex = ExceptionMapper.CreateException(999);
            Assert.IsType<UnknownError>(ex);
        }
    }
}
