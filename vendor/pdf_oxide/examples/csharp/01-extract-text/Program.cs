// Extract text from every page of a PDF and print it.
// Run: dotnet run -- document.pdf

using PdfOxide.Core;

if (args.Length < 1)
{
    Console.Error.WriteLine("Usage: dotnet run -- <file.pdf>");
    return 1;
}

var path = args[0];
using var doc = PdfDocument.Open(path);

Console.WriteLine($"Opened: {path}");
Console.WriteLine($"Pages: {doc.PageCount}\n");

for (int i = 0; i < doc.PageCount; i++)
{
    var text = doc.ExtractText(i);
    Console.WriteLine($"--- Page {i + 1} ---");
    Console.WriteLine($"{text}\n");
}

return 0;
