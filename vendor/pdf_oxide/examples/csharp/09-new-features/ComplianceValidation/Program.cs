// PDF/A, PDF/X, PDF/UA compliance validation
// Run: dotnet run
using System;
using PdfOxide.Core;

Console.WriteLine("Demonstrating PDF/A, PDF/X, PDF/UA compliance validation...");

using var builder = DocumentBuilder.Create();
builder.Title("Compliance Validation Demo");
builder.LetterPage()
    .Font("Helvetica", 12)
    .At(72, 720).Heading(1, "Compliance Validation")
    .At(72, 690).Paragraph("Testing PDF/A, PDF/X, and PDF/UA compliance validators.")
    .Done();
byte[] pdfBytes = builder.Build();

using var doc = PdfDocument.Open(pdfBytes);

Console.WriteLine("Validating PDF/A-2b compliance...");
try
{
    var result = PdfValidator.ValidatePdfA(doc, PdfALevel.A2b);
    Console.WriteLine($"  is_compliant: {result.IsCompliant}");
    Console.WriteLine($"  errors:   [{string.Join(", ", result.Errors)}]");
    Console.WriteLine($"  warnings: [{string.Join(", ", result.Warnings)}]");
}
catch (Exception ex)
{
    Console.WriteLine($"  skipped or errored: {ex.Message}");
}

Console.WriteLine("Validating PDF/X-4 compliance...");
try
{
    var result = PdfValidator.ValidatePdfX(doc, PdfXLevel.X4);
    Console.WriteLine($"  is_compliant: {result.IsCompliant}");
    Console.WriteLine($"  errors:   [{string.Join(", ", result.Errors)}]");
}
catch (Exception ex)
{
    Console.WriteLine($"  skipped or errored: {ex.Message}");
}

Console.WriteLine("Validating PDF/UA-1 compliance...");
try
{
    var result = PdfValidator.ValidatePdfUA(doc, PdfUaLevel.Ua1);
    Console.WriteLine($"  is_compliant: {result.IsCompliant}");
    Console.WriteLine($"  errors:   [{string.Join(", ", result.Errors)}]");
}
catch (Exception ex)
{
    Console.WriteLine($"  skipped or errored: {ex.Message}");
}
