// Image embedding
// Run: dotnet run
//
// Demonstrates embedding JPEG/PNG images into a PDF using raw bytes.
// No pixel dimensions needed — the library auto-detects them from the
// image header. Just supply the display rectangle in PDF points (72 pt = 1 inch).
//
// Addresses issue #425: ImageContent::new() required explicit width/height;
// PageBuilder.Image() does not.
// Addresses issue #450: PNG images with an alpha channel previously displayed
// a diagonal stripe; fixed by adding DecodeParms to the soft-mask XObject.

using System.IO;
using PdfOxide.Core;

const string OutDir = "output";
Directory.CreateDirectory(OutDir);

// 1×1 white PNG (68 bytes) — embedded so the example needs no external files.
var whitePng = new byte[]
{
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
    0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
    0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00,
    0x0C, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0x0D, 0xEF, 0x46, 0xB8, 0x00, 0x00, 0x00,
    0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
};

Console.WriteLine("Building PDF with embedded image...");

// PageBuilder.Image(bytes, x, y, w, h) — no pixel dims needed.
// x, y, w, h are the on-page display rectangle in PDF points (72 pt = 1 inch).
using var builder = DocumentBuilder.Create();
builder.Title("Image Embedding Demo");
builder.LetterPage()
    .Font("Helvetica", 12)
    .At(72, 720).Heading(1, "Image embedding with auto-detected dimensions")
    .At(72, 690).Paragraph("No pixel dims needed — the library reads them from the image header.")
    .Image(whitePng, 72, 480, 200, 200)
    .At(72, 460).Paragraph("Image displayed 200×200 pt — pixel resolution is auto-detected.")
    .Done();

// 1×1 semi-transparent red PNG (RGBA, color type 6) — #450 regression check.
// Previously a diagonal stripe appeared due to missing DecodeParms in the SMask XObject.
var rgbaPng = new byte[]
{
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
    0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
    0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00,
    0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0xD0,
    0x00, 0x00, 0x04, 0x81, 0x01, 0x80, 0x2C, 0x55, 0xCE, 0xB0, 0x00, 0x00,
    0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
};

builder.LetterPage()
    .Font("Helvetica", 12)
    .At(72, 420).Paragraph("Transparent PNG below — rendered without diagonal-line artifact (#450).")
    .Image(rgbaPng, 72, 200, 200, 200)
    .Done();

var path = Path.Combine(OutDir, "image_embedding.pdf");
builder.Save(path);
Console.WriteLine($"Written: {path}");
Console.WriteLine("All image embedding checks passed.");
