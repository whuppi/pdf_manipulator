using System;
using System.IO;
using PdfOxide.Exceptions;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// End-to-end tests that create a PDF on disk, open it, and exercise the
    /// main extraction + editing APIs. Every test is self-contained: it builds
    /// its own input via <see cref="PdfOxide.Core.Pdf.FromMarkdown"/> and cleans
    /// up after itself, so the suite has no fixture-file dependency.
    /// </summary>
    public class PdfDocumentTests
    {
        private static string CreateTestPdf(string markdown)
        {
            using var pdf = PdfOxide.Core.Pdf.FromMarkdown(markdown);
            var path = Path.Combine(Path.GetTempPath(), $"pdfoxide-test-{Guid.NewGuid():N}.pdf");
            pdf.Save(path);
            return path;
        }

        [Fact]
        public void Open_NonExistentFile_Throws()
        {
            Assert.ThrowsAny<PdfException>(() => PdfOxide.Core.PdfDocument.Open("/nonexistent/path/to/file.pdf"));
        }

        [Fact]
        public void RoundTrip_CreateOpenExtract()
        {
            var path = CreateTestPdf("# Hello World\n\nThis is a test PDF with searchable content.");
            try
            {
                using var doc = PdfOxide.Core.PdfDocument.Open(path);
                Assert.True(doc.PageCount >= 1, "PageCount should be at least 1");

                var text = doc.ExtractText(0);
                Assert.False(string.IsNullOrEmpty(text), "ExtractText should return non-empty text");
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        [Fact]
        public void PageCount_ReturnsAtLeastOne()
        {
            var path = CreateTestPdf("# Test\n\nSome content.");
            try
            {
                using var doc = PdfOxide.Core.PdfDocument.Open(path);
                Assert.True(doc.PageCount >= 1);
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        [Fact]
        public void Disposed_DocumentThrowsOnAccess()
        {
            var path = CreateTestPdf("# Disposed\n\nTest.");
            PdfOxide.Core.PdfDocument doc;
            try
            {
                doc = PdfOxide.Core.PdfDocument.Open(path);
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
            doc.Dispose();

            Assert.Throws<ObjectDisposedException>(() => doc.PageCount);
        }

        [Fact]
        public void Dispose_IsIdempotent()
        {
            var path = CreateTestPdf("# Dispose\n\nTest.");
            try
            {
                var doc = PdfOxide.Core.PdfDocument.Open(path);
                doc.Dispose();
                // Second dispose must not throw
                doc.Dispose();
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        [Fact]
        public void Using_AutoDisposes()
        {
            var path = CreateTestPdf("# Using\n\nTest.");
            try
            {
                PdfOxide.Core.PdfDocument? captured;
                using (var doc = PdfOxide.Core.PdfDocument.Open(path))
                {
                    captured = doc;
                    _ = doc.PageCount;
                }
                // After `using` block, the document is disposed and must throw.
                Assert.Throws<ObjectDisposedException>(() => captured!.PageCount);
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        // The following tests exercise P/Invoke entry points that were previously
        // mis-mapped to non-existent Rust symbols (regression protection for the
        // NativeMethods.cs EntryPoint fix).

        [Fact]
        public void Version_ReturnsValidPdfVersion()
        {
            var path = CreateTestPdf("# Version\n\nTest.");
            try
            {
                using var doc = PdfOxide.Core.PdfDocument.Open(path);
                var version = doc.Version;
                Assert.True(version.Major >= 1, $"Expected major >= 1, got {version.Major}");
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        [Fact]
        public void HasStructureTree_DoesNotThrow()
        {
            var path = CreateTestPdf("# Struct\n\nTest.");
            try
            {
                using var doc = PdfOxide.Core.PdfDocument.Open(path);
                _ = doc.HasStructureTree;
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        [Fact]
        public void ToMarkdownAll_ReturnsNonEmpty()
        {
            var path = CreateTestPdf("# Markdown All\n\nBody content.");
            try
            {
                using var doc = PdfOxide.Core.PdfDocument.Open(path);
                var markdown = doc.ToMarkdownAll();
                Assert.False(string.IsNullOrEmpty(markdown));
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        [Fact]
        public void OpenFromStream_Works()
        {
            using var pdf = PdfOxide.Core.Pdf.FromMarkdown("# Bytes\n\nContent.");
            var bytes = pdf.SaveToBytes();
            using var ms = new MemoryStream(bytes);
            using var doc = PdfOxide.Core.PdfDocument.Open(ms);
            Assert.True(doc.PageCount >= 1);
        }

        [Fact]
        public void ExtractImages_OnPageWithNoImages_ReturnsEmptyArray()
        {
            var path = CreateTestPdf("# No images\n\nPlain text only.");
            try
            {
                using var doc = PdfOxide.Core.PdfDocument.Open(path);
                var images = doc.ExtractImages(0);
                Assert.NotNull(images);
                Assert.Empty(images);
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        [Fact]
        public void GetFormFields_OnPlainDocument_ReturnsEmptyArray()
        {
            var path = CreateTestPdf("# No forms\n\nPlain text.");
            try
            {
                using var doc = PdfOxide.Core.PdfDocument.Open(path);
                var fields = doc.GetFormFields();
                Assert.NotNull(fields);
                Assert.Empty(fields);
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        [Fact]
        public void DocumentEditor_SetFormFieldValue_NonExistentField_DoesNotCrash()
        {
            var path = CreateTestPdf("# Editor\n\nForm test.");
            try
            {
                using var editor = PdfOxide.Core.DocumentEditor.Open(path);
                // Setting a field that doesn't exist may throw — but must not crash the runtime.
                // We just exercise the FFI path to confirm the symbol is wired up.
                try { editor.SetFormFieldValue("nonexistent", "x"); } catch (PdfException) { }
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        [Fact]
        public void DocumentEditor_FlattenForms_OnNonFormDoc_DoesNotCrash()
        {
            var path = CreateTestPdf("# Editor\n\nFlatten test.");
            try
            {
                using var editor = PdfOxide.Core.DocumentEditor.Open(path);
                // Flatten on a document with no forms should be a no-op (not crash).
                try { editor.FlattenForms(); } catch (PdfException) { }
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }
    }

    public class PdfCreatorTests
    {
        [Fact]
        public void FromMarkdown_ProducesNonEmptyBytes()
        {
            using var pdf = PdfOxide.Core.Pdf.FromMarkdown("# Markdown\n\nBody text.");
            var bytes = pdf.SaveToBytes();
            Assert.NotNull(bytes);
            Assert.True(bytes.Length > 100, $"Expected at least 100 bytes of PDF output, got {bytes.Length}");
            // PDF magic header
            Assert.Equal((byte)'%', bytes[0]);
            Assert.Equal((byte)'P', bytes[1]);
            Assert.Equal((byte)'D', bytes[2]);
            Assert.Equal((byte)'F', bytes[3]);
        }

        [Fact]
        public void FromText_ProducesValidPdf()
        {
            using var pdf = PdfOxide.Core.Pdf.FromText("Simple plain text content.\nSecond line.");
            var bytes = pdf.SaveToBytes();
            Assert.True(bytes.Length > 100);
        }

        [Fact]
        public void FromHtml_ProducesValidPdf()
        {
            using var pdf = PdfOxide.Core.Pdf.FromHtml("<h1>Title</h1><p>Paragraph</p>");
            var bytes = pdf.SaveToBytes();
            Assert.True(bytes.Length > 100);
        }

        [Fact]
        public void Save_MultipleCallsDoNotDoubleFree()
        {
            // Regression test for a double-free bug in pdf_save's FFI
            // implementation (Box::from_raw consuming the handle).
            using var pdf = PdfOxide.Core.Pdf.FromMarkdown("# Multi-save\n\nContent.");
            var path1 = Path.Combine(Path.GetTempPath(), $"ds1-{Guid.NewGuid():N}.pdf");
            var path2 = Path.Combine(Path.GetTempPath(), $"ds2-{Guid.NewGuid():N}.pdf");
            try
            {
                pdf.Save(path1);
                pdf.Save(path2);
                Assert.True(new FileInfo(path1).Length > 0);
                Assert.True(new FileInfo(path2).Length > 0);
            }
            finally
            {
                if (File.Exists(path1)) File.Delete(path1);
                if (File.Exists(path2)) File.Delete(path2);
            }
        }
    }
}
