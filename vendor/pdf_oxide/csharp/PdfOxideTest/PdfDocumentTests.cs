using Xunit;
using PdfOxide;

namespace PdfOxideTest
{
    public class PdfDocumentTests
    {
        private const string SamplePdf = "scanned_samples/pride_prejudice.pdf";

        [Fact]
        public void OpenDocument_WithValidPath_Succeeds()
        {
            using var doc = new PdfDocument(SamplePdf);
            Assert.NotNull(doc);
            Assert.False(doc.IsClosed);
        }

        [Fact]
        public void OpenDocument_WithInvalidPath_Throws()
        {
            Assert.Throws<PdfException>(() => new PdfDocument("/nonexistent/path.pdf"));
        }

        [Fact]
        public void GetPageCount_WithValidDocument_ReturnsPositiveNumber()
        {
            using var doc = new PdfDocument(SamplePdf);
            int pageCount = doc.GetPageCount();
            Assert.True(pageCount > 0);
        }

        [Fact]
        public void ExtractText_FromValidPage_ReturnsText()
        {
            using var doc = new PdfDocument(SamplePdf);
            string text = doc.ExtractText(0);
            Assert.False(string.IsNullOrEmpty(text));
        }

        [Fact]
        public void ExtractText_FromInvalidPage_Throws()
        {
            using var doc = new PdfDocument(SamplePdf);
            Assert.Throws<PdfException>(() => doc.ExtractText(9999));
        }

        [Fact]
        public void ToMarkdown_WithValidPage_ReturnsMarkdown()
        {
            using var doc = new PdfDocument(SamplePdf);
            string markdown = doc.ToMarkdown(0);
            Assert.False(string.IsNullOrEmpty(markdown));
        }

        [Fact]
        public void ToHtml_WithValidPage_ReturnsHtml()
        {
            using var doc = new PdfDocument(SamplePdf);
            string html = doc.ToHtml(0);
            Assert.False(string.IsNullOrEmpty(html));
        }

        [Fact]
        public void Close_WithOpenDocument_Succeeds()
        {
            var doc = new PdfDocument(SamplePdf);
            doc.Close();
            Assert.True(doc.IsClosed);
        }

        [Fact]
        public void Operations_OnClosedDocument_Throws()
        {
            var doc = new PdfDocument(SamplePdf);
            doc.Close();
            Assert.Throws<ObjectDisposedException>(() => doc.GetPageCount());
        }

        [Fact]
        public void GetVersion_ReturnsValidVersion()
        {
            using var doc = new PdfDocument(SamplePdf);
            var (major, minor) = doc.GetVersion();
            Assert.True(major >= 1);
        }

        [Fact]
        public void HasStructureTree_ReturnsBoolean()
        {
            using var doc = new PdfDocument(SamplePdf);
            bool hasTree = doc.HasStructureTree();
            Assert.IsType<bool>(hasTree);
        }
    }
}
