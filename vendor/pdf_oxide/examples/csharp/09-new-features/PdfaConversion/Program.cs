// PDF/A conversion: validate → convert → validate
// Run: dotnet run
using System;
using System.IO;
using PdfOxide.Core;

const string OutDir = "output";
Directory.CreateDirectory(OutDir);

Console.WriteLine("Demonstrating PDF/A conversion...");

using var builder = DocumentBuilder.Create();
builder.Title("PDF/A Demo");
builder.LetterPage()
    .Font("Helvetica", 12)
    .At(72, 720).Heading(1, "PDF/A Document")
    .At(72, 690).Paragraph("Converting to PDF/A-2b for archival.")
    .Done();
byte[] pdfBytes = builder.Build();

// Step 1: validate before conversion.
Console.WriteLine("Validating PDF/A-2b before conversion...");
try
{
    using var doc1 = PdfDocument.Open(pdfBytes);
    var pre = PdfValidator.ValidatePdfA(doc1, PdfALevel.A2b);
    Console.WriteLine($"  compliant: {pre.IsCompliant}, errors: {pre.Errors.Count}");
}
catch (Exception ex)
{
    Console.WriteLine($"  skipped: {ex.Message}");
}

// Step 2: convert to PDF/A-2b.
Console.WriteLine("Converting to PDF/A-2b...");
using var editor = DocumentEditor.OpenFromBytes(pdfBytes);
try
{
    editor.ConvertToPdfA(PdfALevel.A2b);
    Console.WriteLine("  conversion succeeded.");
}
catch (Exception ex)
{
    Console.WriteLine($"  conversion note: {ex.Message}");
}
byte[] outBytes = editor.SaveToBytes();
Console.WriteLine($"  output: {outBytes.Length} bytes");

// Step 3: validate after conversion.
Console.WriteLine("Validating PDF/A-2b after conversion...");
try
{
    using var doc2 = PdfDocument.Open(outBytes);
    var post = PdfValidator.ValidatePdfA(doc2, PdfALevel.A2b);
    Console.WriteLine($"  compliant: {post.IsCompliant}, errors: {post.Errors.Count}");
}
catch (Exception ex)
{
    Console.WriteLine($"  skipped: {ex.Message}");
}

var path = Path.Combine(OutDir, "pdfa.pdf");
File.WriteAllBytes(path, outBytes);
Console.WriteLine($"Written: {path}");
