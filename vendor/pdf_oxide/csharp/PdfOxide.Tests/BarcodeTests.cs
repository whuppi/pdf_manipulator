using System;
using PdfOxide.Core;
using PdfOxide.Exceptions;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// Tests for the <see cref="Barcode"/> class — the C# exposure of
    /// the barcode Rust FFI at <c>src/ffi.rs:1974</c>.
    ///
    /// Tests that call the native barcode path are guarded: when the native
    /// library is compiled without the <c>barcodes</c> feature the call
    /// throws <see cref="UnsupportedFeatureException"/> (error code 8) and
    /// the test passes vacuously. Argument-validation tests do not need
    /// guards because C# validates inputs before calling native code.
    /// </summary>
    public class BarcodeTests
    {
        [Fact]
        public void Generate_Code128_RoundTripsData()
        {
            Barcode bc;
            try
            {
                bc = Barcode.Generate("HELLO-CODE128", BarcodeFormat.Code128);
            }
            catch (UnsupportedFeatureException)
            {
                return; // barcodes feature not compiled in
            }
            using (bc)
            {
                Assert.Equal(BarcodeFormat.Code128, bc.Format);
                Assert.Equal("HELLO-CODE128", bc.Data);
                Assert.True(bc.Confidence > 0f);
            }
        }

        [Fact]
        public void Generate_Ean13_DefaultsToConfidence1()
        {
            Barcode bc;
            try
            {
                bc = Barcode.Generate("1234567890128", BarcodeFormat.Ean13);
            }
            catch (UnsupportedFeatureException)
            {
                return;
            }
            using (bc)
            {
                Assert.Equal(BarcodeFormat.Ean13, bc.Format);
                Assert.Equal(1.0f, bc.Confidence);
            }
        }

        [Fact]
        public void ToPng_EmitsPngMagic()
        {
            Barcode bc;
            try
            {
                bc = Barcode.Generate("HELLO", BarcodeFormat.Code128);
            }
            catch (UnsupportedFeatureException)
            {
                return;
            }
            using (bc)
            {
                var png = bc.ToPng();
                Assert.True(png.Length >= 8);
                Assert.Equal(0x89, png[0]);
                Assert.Equal(0x50, png[1]);
                Assert.Equal(0x4E, png[2]);
                Assert.Equal(0x47, png[3]);
            }
        }

        // Argument-validation tests — C# validates before calling native.

        [Fact]
        public void Generate_Null_Throws()
        {
            Assert.Throws<ArgumentNullException>(() => Barcode.Generate(null!));
        }

        [Fact]
        public void Generate_Empty_Throws()
        {
            Assert.Throws<ArgumentException>(() => Barcode.Generate(""));
        }

        [Fact]
        public void Generate_InvalidSize_Throws()
        {
            Assert.Throws<ArgumentException>(() => Barcode.Generate("x", BarcodeFormat.Code128, 0));
        }

        [Fact]
        public void Dispose_IsIdempotent()
        {
            Barcode bc;
            try
            {
                bc = Barcode.Generate("x", BarcodeFormat.Code128);
            }
            catch (UnsupportedFeatureException)
            {
                return;
            }
            bc.Dispose();
            bc.Dispose(); // double-dispose must not throw
            Assert.Throws<ObjectDisposedException>(() => _ = bc.Data);
        }
    }
}
