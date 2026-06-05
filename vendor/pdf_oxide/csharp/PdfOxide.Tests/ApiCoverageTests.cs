using System;
using System.IO;
using System.Linq;
using PdfOxide.Core;
using PdfOxide.Exceptions;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// Broad API coverage tests — one test per public method that wasn't
    /// already covered in PdfDocumentTests.cs or DocumentBuilderTests.cs.
    /// Every test is self-contained: it creates its own PDF from Markdown,
    /// exercises the method, and cleans up.
    /// </summary>
    public class ApiCoverageTests
    {
        // ── helpers ──────────────────────────────────────────────────────────

        private static byte[] MakeSimplePdf(string markdown = "# Hello\n\nWorld.")
        {
            using var pdf = Pdf.FromMarkdown(markdown);
            return pdf.SaveToBytes();
        }

        private static TempFile WriteTempPdf(string markdown = "# Hello\n\nWorld.")
        {
            var bytes = MakeSimplePdf(markdown);
            var path = Path.Combine(Path.GetTempPath(), $"pdfoxide-cov-{Guid.NewGuid():N}.pdf");
            File.WriteAllBytes(path, bytes);
            return new TempFile(path);
        }

        private static bool IsUnsupportedFeature(Exception e) =>
            e is UnsupportedFeatureException ||
            e.Message.Contains("5000") ||
            e.Message.Contains("not compiled") ||
            e.Message.ToLower().Contains("unsupported");

        // ── PdfDocument.Open from path ────────────────────────────────────────

        [Fact]
        public void PdfDocument_Open_From_Path_Returns_Document()
        {
            using var tmp = WriteTempPdf();
            using var doc = PdfDocument.Open(tmp);
            Assert.True(doc.PageCount >= 1);
        }

        // ── Text extraction ───────────────────────────────────────────────────

        [Fact]
        public void ExtractWords_Returns_NonEmpty_Array()
        {
            using var tmp = WriteTempPdf("WORDTEST");
            using var doc = PdfDocument.Open(tmp);
            var words = doc.ExtractWords(0);
            Assert.NotNull(words);
            Assert.True(words.Length > 0);
            Assert.Contains(words, w => w.Text.Contains("WORDTEST"));
        }

        [Fact]
        public void ExtractChars_Returns_NonEmpty_Array()
        {
            using var tmp = WriteTempPdf("CHARTEST");
            using var doc = PdfDocument.Open(tmp);
            var chars = doc.ExtractChars(0);
            Assert.NotNull(chars);
            Assert.True(chars.Length > 0);
            Assert.True(chars[0].W > 0 || chars[0].H > 0);
        }

        [Fact]
        public void ExtractTextLines_Returns_NonEmpty_Array()
        {
            using var tmp = WriteTempPdf("LINETEST");
            using var doc = PdfDocument.Open(tmp);
            var lines = doc.ExtractTextLines(0);
            Assert.NotNull(lines);
            Assert.True(lines.Length > 0);
            Assert.Contains(lines, l => l.Text.Contains("LINETEST"));
        }

        [Fact]
        public void ExtractAllText_Returns_NonEmpty_String()
        {
            using var tmp = WriteTempPdf("ALLTEXTMARKER");
            using var doc = PdfDocument.Open(tmp);
            var text = doc.ExtractAllText();
            Assert.False(string.IsNullOrWhiteSpace(text));
            Assert.Contains("ALLTEXTMARKER", text);
        }

        // ── Conversion ────────────────────────────────────────────────────────

        [Fact]
        public void ToMarkdown_Returns_NonEmpty_String()
        {
            using var tmp = WriteTempPdf("MDMARKER");
            using var doc = PdfDocument.Open(tmp);
            var md = doc.ToMarkdown(0);
            Assert.False(string.IsNullOrWhiteSpace(md));
        }

        [Fact]
        public void ToMarkdownAll_Returns_NonEmpty_String()
        {
            using var tmp = WriteTempPdf();
            using var doc = PdfDocument.Open(tmp);
            var md = doc.ToMarkdownAll();
            Assert.False(string.IsNullOrWhiteSpace(md));
        }

        [Fact]
        public void ToHtml_Returns_Html_With_Tags()
        {
            using var tmp = WriteTempPdf();
            using var doc = PdfDocument.Open(tmp);
            var html = doc.ToHtml(0);
            Assert.False(string.IsNullOrWhiteSpace(html));
            Assert.Contains("<", html);
        }

        [Fact]
        public void ToHtmlAll_Returns_Html_With_Tags()
        {
            using var tmp = WriteTempPdf();
            using var doc = PdfDocument.Open(tmp);
            var html = doc.ToHtmlAll();
            Assert.False(string.IsNullOrWhiteSpace(html));
            Assert.Contains("<", html);
        }

        [Fact]
        public void ToPlainText_Returns_NonEmpty_String()
        {
            using var tmp = WriteTempPdf("PLAINMARKER");
            using var doc = PdfDocument.Open(tmp);
            var text = doc.ToPlainText(0);
            Assert.False(string.IsNullOrWhiteSpace(text));
            Assert.Contains("PLAINMARKER", text);
        }

        [Fact]
        public void ToPlainTextAll_Returns_NonEmpty_String()
        {
            using var tmp = WriteTempPdf("PLAINALL");
            using var doc = PdfDocument.Open(tmp);
            var text = doc.ToPlainTextAll();
            Assert.False(string.IsNullOrWhiteSpace(text));
            Assert.Contains("PLAINALL", text);
        }

        // ── Search ────────────────────────────────────────────────────────────

        [Fact]
        public void SearchPage_Finds_Known_Term()
        {
            using var tmp = WriteTempPdf("SEARCHTOKEN");
            using var doc = PdfDocument.Open(tmp);
            var results = doc.SearchPage(0, "SEARCHTOKEN");
            Assert.True(results.Length > 0);
        }

        [Fact]
        public void SearchAll_Finds_Known_Term()
        {
            using var tmp = WriteTempPdf("SEARCHALLTOKEN");
            using var doc = PdfDocument.Open(tmp);
            var results = doc.SearchAll("SEARCHALLTOKEN");
            Assert.True(results.Length > 0);
        }

        [Fact]
        public void SearchAll_Missing_Term_Returns_Empty()
        {
            using var tmp = WriteTempPdf();
            using var doc = PdfDocument.Open(tmp);
            var results = doc.SearchAll("ZZZNOMATCHZZZ");
            Assert.Empty(results);
        }

        // ── DocumentBuilder extras ────────────────────────────────────────────

        [Fact]
        public void DocumentBuilder_Save_NonEncrypted()
        {
            var path = Path.Combine(Path.GetTempPath(), $"pdfoxide-save-{Guid.NewGuid():N}.pdf");
            try
            {
                using var builder = DocumentBuilder.Create();
                builder.A4Page().Paragraph("plain save test").Done();
                builder.Save(path);
                Assert.True(File.Exists(path));
                Assert.True(new FileInfo(path).Length > 100);
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        [Fact]
        public void DocumentBuilder_LetterPage_Produces_Pdf()
        {
            using var builder = DocumentBuilder.Create();
            builder.LetterPage().Paragraph("US Letter").Done();
            var bytes = builder.Build();
            Assert.StartsWith("%PDF-", System.Text.Encoding.ASCII.GetString(bytes.Take(8).ToArray()));
        }

        [Fact]
        public void DocumentBuilder_CustomPage_Produces_Pdf()
        {
            using var builder = DocumentBuilder.Create();
            builder.Page(300f, 400f).Paragraph("custom size").Done();
            var bytes = builder.Build();
            Assert.True(bytes.Length > 100);
        }

        [Fact]
        public void DocumentBuilder_Metadata_Setters_Do_Not_Throw()
        {
            using var builder = DocumentBuilder.Create()
                .Title("My Title")
                .Author("Alice")
                .Subject("Testing")
                .Keywords("pdf, test")
                .Creator("xunit");
            builder.A4Page().Paragraph("metadata test").Done();
            var bytes = builder.Build();
            Assert.True(bytes.Length > 100);
        }

        [Fact]
        public void DocumentBuilder_ToBytesEncrypted_Produces_Encrypted_Pdf()
        {
            using var builder = DocumentBuilder.Create();
            builder.A4Page().Paragraph("secret").Done();
            var bytes = builder.ToBytesEncrypted("user", "owner");
            Assert.True(bytes.Length > 100);
            var raw = System.Text.Encoding.Latin1.GetString(bytes);
            Assert.Contains("/Encrypt", raw);
        }

        // ── DocumentEditor mutations ──────────────────────────────────────────

        [Fact]
        public void DocumentEditor_DeletePage_Reduces_PageCount()
        {
            // build a 3-page PDF
            using var pdfA = Pdf.FromMarkdown("# P1");
            using var pdfB = Pdf.FromMarkdown("# P2");
            using var pdfC = Pdf.FromMarkdown("# P3");
            var path = Path.Combine(Path.GetTempPath(), $"pdfoxide-edit-{Guid.NewGuid():N}.pdf");
            try
            {
                pdfA.Save(path);
                using var editor = DocumentEditor.Open(path);
                editor.MergeFrom(path); // now 2 pages (P1 + P1)
                int before = editor.PageCount;
                editor.DeletePage(0);
                editor.Save(path);

                using var doc = PdfDocument.Open(path);
                Assert.Equal(before - 1, doc.PageCount);
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        [Fact]
        public void DocumentEditor_MovePage_Changes_Order()
        {
            var srcPath = Path.Combine(Path.GetTempPath(), $"pdfoxide-move-src-{Guid.NewGuid():N}.pdf");
            var outPath = Path.Combine(Path.GetTempPath(), $"pdfoxide-move-out-{Guid.NewGuid():N}.pdf");
            try
            {
                // Use DocumentBuilder to create a 2-page PDF in one shot so all
                // pages live in page_order from the start (no merged_pages split).
                using (var builder = DocumentBuilder.Create())
                {
                    builder.A4Page().At(72, 720).Text("PAGEFIRST").Done();
                    builder.A4Page().At(72, 720).Text("PAGESECOND").Done();
                    File.WriteAllBytes(srcPath, builder.Build());
                }

                using var editor = DocumentEditor.Open(srcPath);
                editor.MovePage(1, 0);   // swap: PAGESECOND, PAGEFIRST
                editor.Save(outPath);

                using var doc = PdfDocument.Open(outPath);
                Assert.Equal(2, doc.PageCount);
                var words = doc.ExtractWords(0);
                Assert.True(words.Any(w => w.Text.Contains("PAGESECOND")),
                    $"Expected 'PAGESECOND' on page 0, got: {string.Join(", ", words.Select(w => w.Text))}");
            }
            finally
            {
                if (File.Exists(srcPath)) File.Delete(srcPath);
                if (File.Exists(outPath)) File.Delete(outPath);
            }
        }

        [Fact]
        public void DocumentEditor_MergeFrom_Increases_PageCount()
        {
            var pathA = Path.Combine(Path.GetTempPath(), $"pdfoxide-ma-{Guid.NewGuid():N}.pdf");
            var pathB = Path.Combine(Path.GetTempPath(), $"pdfoxide-mb-{Guid.NewGuid():N}.pdf");
            var outPath = Path.Combine(Path.GetTempPath(), $"pdfoxide-mout-{Guid.NewGuid():N}.pdf");
            try
            {
                using (var a = Pdf.FromMarkdown("# A")) a.Save(pathA);
                using (var b = Pdf.FromMarkdown("# B")) b.Save(pathB);

                using var editor = DocumentEditor.Open(pathA);
                int before = editor.PageCount;
                editor.MergeFrom(pathB);
                editor.Save(outPath);  // save to separate file to avoid same-file lock

                using var doc = PdfDocument.Open(outPath);
                Assert.True(doc.PageCount > before);
            }
            finally
            {
                if (File.Exists(pathA)) File.Delete(pathA);
                if (File.Exists(pathB)) File.Delete(pathB);
                if (File.Exists(outPath)) File.Delete(outPath);
            }
        }

        // ── Pdf factory extras ────────────────────────────────────────────────

        [Fact]
        public void Pdf_FromText_Produces_Pdf()
        {
            try
            {
                using var pdf = Pdf.FromText("Hello plain text");
                var bytes = pdf.SaveToBytes();
                Assert.StartsWith("%PDF-", System.Text.Encoding.ASCII.GetString(bytes.Take(8).ToArray()));
            }
            catch (Exception e) when (IsUnsupportedFeature(e))
            {
                // skip — feature not compiled in
            }
        }

        // ── Temp-file helper ──────────────────────────────────────────────────

        private readonly struct TempFile : IDisposable
        {
            public string Path { get; }
            public TempFile(string path) => Path = path;
            public static implicit operator string(TempFile t) => t.Path;
            public void Dispose()
            {
                try { File.Delete(Path); } catch { }
            }
        }
    }
}
