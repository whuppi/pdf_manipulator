// Page extraction
// Run: dotnet run
using System;
using System.IO;
using PdfOxide.Core;

const string OutDir = "output";
Directory.CreateDirectory(OutDir);

Console.WriteLine("Demonstrating page extraction...");

using var builder = DocumentBuilder.Create();
builder.Title("Two-Page Document");
builder.LetterPage()
    .Font("Helvetica", 12)
    .At(72, 720).Heading(1, "Page One")
    .At(72, 690).Paragraph("Content on the first page.")
    .Done();
builder.LetterPage()
    .Font("Helvetica", 12)
    .At(72, 720).Heading(1, "Page Two")
    .At(72, 690).Paragraph("Content on the second page.")
    .Done();
byte[] pdfBytes = builder.Build();

using var editor = DocumentEditor.OpenFromBytes(pdfBytes);
Console.WriteLine($"  Source page count: {editor.PageCount}");

byte[] page0Bytes = editor.ExtractPages(new[] { 0 });
Console.WriteLine($"  Extracted page 0: {page0Bytes.Length} bytes");
if (page0Bytes.Length == 0) throw new InvalidOperationException("extracted bytes is empty");

var path = Path.Combine(OutDir, "page_0.pdf");
File.WriteAllBytes(path, page0Bytes);
Console.WriteLine($"Written: {path}");
