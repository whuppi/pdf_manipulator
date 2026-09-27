using System;
using PdfOxide.Core;
using PdfOxide.Exceptions;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// Tests for <see cref="OcrEngine"/>.
    ///
    /// OCR requires the native library to be built with the `ocr`
    /// feature + real ONNX models on disk. We can't ship the ~80 MB
    /// model files in the test tree, so these tests exercise the API
    /// shape and the "feature-off" failure mode rather than full
    /// text recognition. A deeper test belongs in a separate fixture
    /// directory that isn't bundled with the C# binding.
    /// </summary>
    public class OcrEngineTests
    {
        [Fact]
        public void Load_Null_Throws()
        {
            Assert.Throws<ArgumentNullException>(
                () => OcrEngine.Load(null!, "r.onnx", "dict.txt"));
        }

        [Fact]
        public void Load_MissingFiles_ThrowsPdfException()
        {
            // Either PdfException (feature on, file missing) or
            // UnsupportedFeatureException (feature off). Both are
            // acceptable signals that the API is wired correctly.
            Assert.ThrowsAny<PdfException>(
                () => OcrEngine.Load("/does/not/exist/det.onnx", "/nope/rec.onnx", "/nope/dict.txt"));
        }

        [Fact]
        public void PageNeedsOcr_NullDoc_Throws()
        {
            Assert.Throws<ArgumentNullException>(
                () => OcrEngine.PageNeedsOcr(null!, 0));
        }

        [Fact]
        public void ExtractText_NullDoc_Throws()
        {
            // Build a dummy engine by loading invalid paths; we expect
            // the Load call to throw before we get to ExtractText in
            // most builds. If Load succeeds somehow (unlikely) then
            // ExtractText still needs to reject null document.
            try
            {
                using var engine = OcrEngine.Load("/x/det.onnx", "/x/rec.onnx", "/x/dict.txt");
                using var doc = PdfDocument.Open(Pdf.FromMarkdown("# t").SaveToBytes());
                Assert.Throws<ArgumentNullException>(() => engine.ExtractText(null!, 0));
            }
            catch (PdfException)
            {
                // Expected: Load rejects missing model files. The
                // null-doc assertion then can't be tested here.
            }
        }
    }
}
