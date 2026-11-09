// Extract text content from a PDF page.
// Form fields and annotations require FFI calls that are not yet wired into
// the C# binding; use the Rust core or Python binding for those features.
// Run: dotnet run --project csharp/ -- document.pdf

using PdfOxide.Core;

if (args.Length < 1)
{
    Console.Error.WriteLine("Usage: dotnet run -- <document.pdf>");
    return 1;
}

var path = args[0];
using var doc = PdfDocument.Open(path);
Console.WriteLine($"Opened: {path}");
Console.WriteLine($"Pages: {doc.PageCount}");
Console.WriteLine($"PDF version: {doc.Version.Major}.{doc.Version.Minor}");

for (int page = 0; page < doc.PageCount; page++)
{
    var text = doc.ExtractText(page);
    Console.WriteLine($"\n--- Page {page + 1} ({text.Length} characters) ---");
    Console.WriteLine(text.Length > 400 ? text.Substring(0, 400) + "..." : text);
}

return 0;
