using System;
using System.IO;
using PdfOxide.Core;
using PdfOxide.Exceptions;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// Tests for the 12 DocumentEditor mutation methods surfaced in the public
    /// C# API. Each method has an existing P/Invoke in <c>NativeMethods.cs</c>
    /// (<c>document_editor_*</c>); these tests lock down the public wrappers
    /// and argument validation.
    ///
    /// Cross-link: Reddit user u/Raccoon12 (2026-04-21) explicitly asked
    /// "When is DocumentEditor::merge_from going to get added to the .NET
    /// bindings?" — MergeFrom is the headline of this commit.
    /// </summary>
    public class DocumentEditorMutationTests
    {
        // ---------------------------------------------------------------
        // Helpers: build a multi-page test PDF on disk + wrap in editor.
        // ---------------------------------------------------------------
        private static string CreateTestPdf(string markdown)
        {
            using var pdf = Pdf.FromMarkdown(markdown);
            var path = Path.Combine(Path.GetTempPath(), $"pdfoxide-edmut-{Guid.NewGuid():N}.pdf");
            pdf.Save(path);
            return path;
        }

        private static string CreateMultiPagePdf(int pages)
        {
            // Use DocumentBuilder so each added page is guaranteed.
            // Pdf.FromMarkdown collapses short content onto a single page,
            // which made earlier attempts flaky.
            using var builder = DocumentBuilder.Create();
            for (int i = 0; i < pages; i++)
            {
                builder.A4Page()
                       .At(72, 720)
                       .Text($"Page {i + 1}")
                       .Done();
            }
            var bytes = builder.Build();
            var path = Path.Combine(Path.GetTempPath(), $"pdfoxide-edmut-multi-{Guid.NewGuid():N}.pdf");
            File.WriteAllBytes(path, bytes);
            return path;
        }

        // ---------------------------------------------------------------
        // MergeFrom — requested by u/Raccoon12 on Reddit.
        // ---------------------------------------------------------------
        [Fact]
        public void MergeFrom_AppendsPages()
        {
            var a = CreateTestPdf("# A\n\nContent.");
            var b = CreateTestPdf("# B\n\nSecond.");
            try
            {
                using var editor = DocumentEditor.Open(a);
                var before = editor.PageCount;
                editor.MergeFrom(b);
                Assert.True(editor.PageCount >= before);
                Assert.True(editor.IsModified);
            }
            finally
            {
                File.Delete(a);
                File.Delete(b);
            }
        }

        [Fact]
        public void MergeFrom_NullPath_Throws()
        {
            var a = CreateTestPdf("# A");
            try
            {
                using var editor = DocumentEditor.Open(a);
                Assert.Throws<ArgumentNullException>(() => editor.MergeFrom(null!));
            }
            finally { File.Delete(a); }
        }

        // ---------------------------------------------------------------
        // DeletePage
        // ---------------------------------------------------------------
        [Fact]
        public void DeletePage_ReducesPageCount()
        {
            var path = CreateMultiPagePdf(2);
            try
            {
                using var editor = DocumentEditor.Open(path);
                var before = editor.PageCount;
                editor.DeletePage(0);
                Assert.Equal(before - 1, editor.PageCount);
            }
            finally { File.Delete(path); }
        }

        // ---------------------------------------------------------------
        // MovePage
        // ---------------------------------------------------------------
        [Fact]
        public void MovePage_PreservesPageCount()
        {
            var path = CreateMultiPagePdf(3);
            try
            {
                using var editor = DocumentEditor.Open(path);
                var before = editor.PageCount;
                editor.MovePage(0, 2);
                Assert.Equal(before, editor.PageCount);
                Assert.True(editor.IsModified);
            }
            finally { File.Delete(path); }
        }

        // ---------------------------------------------------------------
        // Page rotation
        // ---------------------------------------------------------------
        [Fact]
        public void SetPageRotation_RoundTrips()
        {
            var path = CreateTestPdf("# Rotate");
            try
            {
                using var editor = DocumentEditor.Open(path);
                editor.SetPageRotation(0, 90);
                Assert.Equal(90, editor.GetPageRotation(0));
            }
            finally { File.Delete(path); }
        }

        // ---------------------------------------------------------------
        // CropMargins
        // ---------------------------------------------------------------
        [Fact]
        public void CropMargins_MarksModified()
        {
            var path = CreateTestPdf("# Crop");
            try
            {
                using var editor = DocumentEditor.Open(path);
                editor.CropMargins(10f, 10f, 10f, 10f);
                Assert.True(editor.IsModified);
            }
            finally { File.Delete(path); }
        }

        // ---------------------------------------------------------------
        // EraseRegion
        // ---------------------------------------------------------------
        [Fact]
        public void EraseRegion_MarksModified()
        {
            var path = CreateTestPdf("# Erase");
            try
            {
                using var editor = DocumentEditor.Open(path);
                editor.EraseRegion(0, 100f, 100f, 50f, 20f);
                Assert.True(editor.IsModified);
            }
            finally { File.Delete(path); }
        }

        // ---------------------------------------------------------------
        // FlattenAnnotations (per-page) + FlattenAllAnnotations
        // ---------------------------------------------------------------
        [Fact]
        public void FlattenAnnotations_OnPage_DoesNotThrow()
        {
            var path = CreateTestPdf("# No annots");
            try
            {
                using var editor = DocumentEditor.Open(path);
                editor.FlattenAnnotations(0);
            }
            finally { File.Delete(path); }
        }

        [Fact]
        public void FlattenAllAnnotations_DoesNotThrow()
        {
            var path = CreateTestPdf("# No annots");
            try
            {
                using var editor = DocumentEditor.Open(path);
                editor.FlattenAllAnnotations();
            }
            finally { File.Delete(path); }
        }

        [Fact]
        public void FlattenFormsOnPage_DoesNotThrow()
        {
            var path = CreateTestPdf("# No forms");
            try
            {
                using var editor = DocumentEditor.Open(path);
                editor.FlattenFormsOnPage(0);
            }
            finally { File.Delete(path); }
        }

        // ---------------------------------------------------------------
        // SaveEncrypted
        // ---------------------------------------------------------------
        [Fact]
        public void SaveEncrypted_ProducesAes256Dict()
        {
            var path = CreateTestPdf("# secret");
            var outPath = Path.Combine(Path.GetTempPath(), $"pdfoxide-enc-{Guid.NewGuid():N}.pdf");
            try
            {
                using (var editor = DocumentEditor.Open(path))
                    editor.SaveEncrypted(outPath, "user-pw", "owner-pw");

                var bytes = File.ReadAllBytes(outPath);
                var text = System.Text.Encoding.ASCII.GetString(bytes);
                Assert.Contains("/Encrypt", text);
                Assert.Contains("/V 5", text);
            }
            finally
            {
                File.Delete(path);
                if (File.Exists(outPath)) File.Delete(outPath);
            }
        }

        [Fact]
        public void SaveEncrypted_NullArg_Throws()
        {
            var path = CreateTestPdf("# secret");
            try
            {
                using var editor = DocumentEditor.Open(path);
                Assert.Throws<ArgumentNullException>(() => editor.SaveEncrypted(null!, "u", "o"));
            }
            finally { File.Delete(path); }
        }

        // ---------------------------------------------------------------
        // Producer / CreationDate metadata round-trip
        // ---------------------------------------------------------------
        // Producer / CreationDate round-trip against the Rust core.
        // Set → save → reopen → read back.
        [Fact]
        public void Producer_RoundTrips_AfterSave()
        {
            var path = CreateTestPdf("# Meta");
            var outPath = Path.Combine(Path.GetTempPath(), $"pdfoxide-prod-{Guid.NewGuid():N}.pdf");
            try
            {
                using (var editor = DocumentEditor.Open(path))
                {
                    editor.Producer = "PdfOxide Unit-test";
                    editor.Save(outPath);
                }

                using var reopened = DocumentEditor.Open(outPath);
                Assert.Equal("PdfOxide Unit-test", reopened.Producer);
            }
            finally
            {
                File.Delete(path);
                if (File.Exists(outPath)) File.Delete(outPath);
            }
        }

        [Fact]
        public void Producer_Setter_PersistsToOutputBytes_Debug()
        {
            var path = CreateTestPdf("# Meta");
            var outPath = Path.Combine(Path.GetTempPath(), $"pdfoxide-probe-{Guid.NewGuid():N}.pdf");
            try
            {
                using (var editor = DocumentEditor.Open(path))
                {
                    editor.Producer = "PROBE-STRING-X";
                    Assert.True(editor.IsModified, "IsModified must be true after set_producer");
                    editor.Save(outPath);
                }
                var saved = File.ReadAllBytes(outPath);
                var ascii = System.Text.Encoding.ASCII.GetString(saved);
                Assert.Contains("/Producer", ascii);
                Assert.Contains("PROBE-STRING-X", ascii);
            }
            finally
            {
                File.Delete(path);
                if (File.Exists(outPath)) File.Delete(outPath);
            }
        }

        [Fact]
        public void CreationDate_RoundTrips_AfterSave()
        {
            var path = CreateTestPdf("# Meta");
            var outPath = Path.Combine(Path.GetTempPath(), $"pdfoxide-cd-{Guid.NewGuid():N}.pdf");
            try
            {
                using (var editor = DocumentEditor.Open(path))
                {
                    editor.CreationDate = "D:20260421120000Z";
                    editor.Save(outPath);
                }

                using var reopened = DocumentEditor.Open(outPath);
                Assert.Equal("D:20260421120000Z", reopened.CreationDate);
            }
            finally
            {
                File.Delete(path);
                if (File.Exists(outPath)) File.Delete(outPath);
            }
        }
    }
}
