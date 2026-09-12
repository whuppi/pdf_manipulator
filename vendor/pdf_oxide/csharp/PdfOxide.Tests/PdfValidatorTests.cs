using System;
using System.IO;
using PdfOxide.Core;
using PdfOxide.Exceptions;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// Tests for <see cref="PdfValidator"/>.
    /// Smoke-level only: confirms the validator returns a non-null
    /// result and that compliance / error lists are populated in a
    /// sane way for plain markdown-generated PDFs (which are
    /// NOT PDF/A nor PDF/X nor PDF/UA compliant).
    /// </summary>
    public class PdfValidatorTests
    {
        private static PdfDocument CreateTestDoc()
        {
            using var pdf = Pdf.FromMarkdown("# Validate me\n\nBody.");
            var bytes = pdf.SaveToBytes();
            return PdfDocument.Open(bytes);
        }

        [Fact]
        public void ValidatePdfA_ReturnsResult()
        {
            using var doc = CreateTestDoc();
            var r = PdfValidator.ValidatePdfA(doc, PdfALevel.A2b);
            Assert.NotNull(r);
            // Markdown-generated PDFs don't include PDF/A metadata,
            // so the validator should report non-compliance.
            Assert.False(r.IsCompliant);
            Assert.NotEmpty(r.Errors);
        }

        [Fact]
        public void ValidatePdfX_ReturnsResult()
        {
            using var doc = CreateTestDoc();
            var r = PdfValidator.ValidatePdfX(doc, PdfXLevel.X4);
            Assert.NotNull(r);
            Assert.False(r.IsCompliant);
        }

        [Fact]
        public void ValidatePdfUA_ReturnsResult()
        {
            using var doc = CreateTestDoc();
            var r = PdfValidator.ValidatePdfUA(doc, PdfUaLevel.Ua1);
            Assert.NotNull(r);
        }

        [Fact]
        public void ValidatePdfA_NullDoc_Throws()
        {
            Assert.Throws<ArgumentNullException>(() => PdfValidator.ValidatePdfA(null!));
        }
    }
}
