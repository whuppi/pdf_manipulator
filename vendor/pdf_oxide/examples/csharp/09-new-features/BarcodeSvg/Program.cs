// Barcode SVG generation
// Run: dotnet run

using System;
using System.IO;
using PdfOxide.Core;

const string OutDir = "output";
Directory.CreateDirectory(OutDir);

// 1D barcode — Code 128 SVG
using var code128 = Barcode.Generate("PDF-OXIDE-0341", BarcodeFormat.Code128, 300);
string svg = code128.ToSvg();
if (!svg.StartsWith("<svg", StringComparison.Ordinal))
    throw new Exception($"Expected SVG output for Code128, got: {svg[..Math.Min(40, svg.Length)]}");
var path = Path.Combine(OutDir, "code128.svg");
File.WriteAllText(path, svg);
Console.WriteLine($"Written: {path} ({svg.Length} bytes)");

// 1D barcode — EAN-13 SVG
using var ean13 = Barcode.Generate("5901234123457", BarcodeFormat.Ean13, 300);
svg = ean13.ToSvg();
if (!svg.StartsWith("<svg", StringComparison.Ordinal))
    throw new Exception("Expected SVG output for EAN-13");
path = Path.Combine(OutDir, "ean13.svg");
File.WriteAllText(path, svg);
Console.WriteLine($"Written: {path} ({svg.Length} bytes)");

// QR code SVG
using var qr = Barcode.GenerateQrCode("https://github.com/yfedoseev/pdf_oxide", errorCorrection: 1, sizePx: 256);
svg = qr.ToSvg();
if (!svg.StartsWith("<svg", StringComparison.Ordinal))
    throw new Exception("Expected SVG output for QR code");
if (!svg.Contains("<rect"))
    throw new Exception("QR SVG must contain rect elements");
path = Path.Combine(OutDir, "qr_code.svg");
File.WriteAllText(path, svg);
Console.WriteLine($"Written: {path} ({svg.Length} bytes)");

Console.WriteLine("All barcode SVG checks passed.");
