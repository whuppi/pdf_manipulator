// In-memory round-trip: Build() → bytes → PdfDocument.Open(bytes)
// Run: dotnet run

using System;
using System.IO;
using PdfOxide.Core;

const string OutDir = "output";
Directory.CreateDirectory(OutDir);

Console.WriteLine("Demonstrating in-memory round-trip...");

using var builder = DocumentBuilder.Create();
builder.Title("In-Memory Round-Trip Demo");
builder.LetterPage()
    .Font("Helvetica", 12)
    .At(72, 720).Heading(1, "In-Memory Round-Trip")
    .At(72, 690).Paragraph("This PDF was built in memory, never written to disk mid-way.")
    .Done();

byte[] pdfBytes = builder.Build();

using var doc = PdfDocument.Open(pdfBytes);
string text = doc.ExtractText(0);
Console.WriteLine($"  Extracted {text.Length} chars from in-memory PDF");
if (!text.Contains("In-Memory"))
    throw new InvalidOperationException("round-trip text missing");

var path = Path.Combine(OutDir, "in_memory_roundtrip.pdf");
File.WriteAllBytes(path, pdfBytes);
Console.WriteLine($"Written: {path}");
