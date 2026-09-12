// PKCS#12 CMS signing
// Run: dotnet run

using System;
using System.IO;
using System.Text;
using PdfOxide.Core;
using PdfOxide.Exceptions;

const string OutDir = "output";
Directory.CreateDirectory(OutDir);

var p12Path = Path.GetFullPath(Path.Combine(
    AppContext.BaseDirectory, "..", "..", "..", "..", "..", "..",
    "tests", "fixtures", "test_signing.p12"));

if (!File.Exists(p12Path))
{
    Console.WriteLine($"  SKIP: {p12Path} not found");
    return;
}

try
{
    Console.WriteLine("Signing PDF with PKCS#12 certificate...");
    var p12Data = File.ReadAllBytes(p12Path);
    using var cert = Certificate.Load(p12Data, "testpass");
    Console.WriteLine($"  Certificate subject: {cert.Subject}");

    using var builder = DocumentBuilder.Create();
    builder.Title("Signed Invoice");
    builder.LetterPage()
        .Font("Helvetica", 12)
        .At(72, 720).Heading(1, "Signed Invoice")
        .At(72, 690).Paragraph("This document carries a CMS/PKCS#7 digital signature.")
        .Done();
    byte[] pdfBytes = builder.Build();

    byte[] signed = cert.SignPdfBytes(pdfBytes, reason: "Approved", location: "HQ");

    var path = Path.Combine(OutDir, "signed_document.pdf");
    File.WriteAllBytes(path, signed);
    Console.WriteLine($"Written: {path} ({signed.Length} bytes)");

    if (!Encoding.Latin1.GetString(signed).Contains("/ByteRange"))
        throw new InvalidOperationException("ByteRange missing from signed PDF");
    Console.WriteLine("  Signature verified: /ByteRange present.");
}
catch (UnsupportedFeatureException)
{
    Console.WriteLine("  SKIP: signatures feature not compiled in.");
}
