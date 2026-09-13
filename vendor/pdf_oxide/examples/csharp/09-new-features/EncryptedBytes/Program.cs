// Encrypted PDF bytes
// Run: dotnet run
using System;
using System.IO;
using PdfOxide.Core;

const string OutDir = "output";
Directory.CreateDirectory(OutDir);

Console.WriteLine("Demonstrating encrypted in-memory PDF...");

using var builder = DocumentBuilder.Create();
builder.Title("Encrypted PDF Demo");
builder.LetterPage()
    .Font("Helvetica", 12)
    .At(72, 720).Heading(1, "Encrypted PDF")
    .At(72, 690).Paragraph("This PDF is encrypted with AES-256.")
    .Done();
byte[] pdfBytes = builder.Build();

using var editor = DocumentEditor.OpenFromBytes(pdfBytes);
byte[] encrypted = editor.SaveEncryptedToBytes("user123", "owner123");
Console.WriteLine($"  Original: {pdfBytes.Length} bytes → Encrypted: {encrypted.Length} bytes");
if (encrypted.Length == 0) throw new InvalidOperationException("encrypted bytes is empty");

var path = Path.Combine(OutDir, "encrypted.pdf");
File.WriteAllBytes(path, encrypted);
Console.WriteLine($"Written: {path}");
