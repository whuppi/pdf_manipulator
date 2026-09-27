using System;
using System.IO;
using PdfOxide.Core;
using PdfOxide.Exceptions;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// Verifies the byte-array and <see cref="ReadOnlySpan{T}"/> Open
    /// overloads introduced to avoid the MemoryStream copy in the
    /// Stream-based overload.
    ///
    /// Feature request: Reddit user u/gevorgter (2026-04-21) —
    ///   > "Also, may be add another method.
    ///   >  public static PdfDocument Open(byte[] pdf)
    ///   >  or better yet, Span&lt;byte&gt;"
    /// </summary>
    public class OpenFromBytesTests
    {
        private static byte[] CreateTestPdfBytes(string markdown = "# Test\n\nHello.")
        {
            using var pdf = Pdf.FromMarkdown(markdown);
            return pdf.SaveToBytes();
        }

        [Fact]
        public void Open_ByteArray_Succeeds()
        {
            var bytes = CreateTestPdfBytes();
            using var doc = PdfDocument.Open(bytes);
            Assert.True(doc.PageCount >= 1);
        }

        [Fact]
        public void Open_ReadOnlySpan_Succeeds()
        {
            var bytes = CreateTestPdfBytes();
            using var doc = PdfDocument.Open((ReadOnlySpan<byte>)bytes);
            Assert.True(doc.PageCount >= 1);
        }

        [Fact]
        public void Open_ByteArray_Null_Throws()
        {
            Assert.Throws<ArgumentNullException>(() => PdfDocument.Open((byte[])null!));
        }

        [Fact]
        public void Open_ByteArray_Empty_Throws()
        {
            Assert.Throws<ArgumentException>(() => PdfDocument.Open(Array.Empty<byte>()));
        }

        [Fact]
        public void Open_ReadOnlySpan_Empty_Throws()
        {
            Assert.Throws<ArgumentException>(() => PdfDocument.Open(ReadOnlySpan<byte>.Empty));
        }

        [Fact]
        public void Open_ByteArray_InvalidData_ThrowsPdfException()
        {
            var garbage = new byte[] { 0x00, 0x01, 0x02, 0x03 };
            Assert.ThrowsAny<PdfException>(() => PdfDocument.Open(garbage));
        }

        [Fact]
        public void Open_ByteArray_RoundTripMatches_StreamPath()
        {
            var bytes = CreateTestPdfBytes("# Round-trip\n\nEquivalence.");
            using var fromBytes = PdfDocument.Open(bytes);
            using var fromStream = PdfDocument.Open(new MemoryStream(bytes));
            Assert.Equal(fromStream.PageCount, fromBytes.PageCount);
        }
    }
}
