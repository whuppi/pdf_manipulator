using System;
using System.IO;
using PdfOxide.Core;
using PdfOxide.Exceptions;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// Verifies the static factories that build a single-page PDF around a
    /// raster image (JPEG / PNG). Mirrors Python's <c>Pdf.from_image</c> and
    /// <c>Pdf.from_image_bytes</c>; was missing from C# per audit §3.
    /// </summary>
    public class PdfFromImageTests
    {
        // Minimal valid 1×1 red-pixel PNG (PIL-generated, CRC-verified).
        private static readonly byte[] OnePixelPng = Convert.FromBase64String(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC");

        [Fact]
        public void FromImageBytes_Png_Succeeds()
        {
            using var pdf = Pdf.FromImageBytes(OnePixelPng);
            var bytes = pdf.SaveToBytes();
            Assert.True(bytes.Length > 0);
            using var doc = PdfDocument.Open(bytes);
            Assert.True(doc.PageCount >= 1);
        }

        [Fact]
        public void FromImage_Path_Succeeds()
        {
            var tmp = Path.Combine(Path.GetTempPath(), $"pdfoxide-img-{Guid.NewGuid():N}.png");
            File.WriteAllBytes(tmp, OnePixelPng);
            try
            {
                using var pdf = Pdf.FromImage(tmp);
                var bytes = pdf.SaveToBytes();
                Assert.True(bytes.Length > 0);
            }
            finally
            {
                File.Delete(tmp);
            }
        }

        [Fact]
        public void FromImageBytes_Null_Throws()
        {
            Assert.Throws<ArgumentNullException>(() => Pdf.FromImageBytes(null!));
        }

        [Fact]
        public void FromImageBytes_Empty_Throws()
        {
            Assert.Throws<ArgumentException>(() => Pdf.FromImageBytes(Array.Empty<byte>()));
        }

        [Fact]
        public void FromImage_Null_Throws()
        {
            Assert.Throws<ArgumentNullException>(() => Pdf.FromImage(null!));
        }

        [Fact]
        public void FromImageBytes_Garbage_ThrowsPdfException()
        {
            var garbage = new byte[] { 0x00, 0x01, 0x02, 0x03, 0x04, 0x05 };
            Assert.ThrowsAny<PdfException>(() => Pdf.FromImageBytes(garbage));
        }
    }
}
