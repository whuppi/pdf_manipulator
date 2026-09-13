// Convert PDF pages to Markdown, HTML, and plain text files.
// Run: dotnet run -- document.pdf

using PdfOxide.Core;

if (args.Length < 1)
{
    Console.Error.WriteLine("Usage: dotnet run -- <file.pdf>");
    return 1;
}

var path = args[0];
using var doc = PdfDocument.Open(path);

Directory.CreateDirectory("output");
var pages = doc.PageCount;
Console.WriteLine($"Converting {pages} pages from {path}...");

for (int i = 0; i < pages; i++)
{
    var n = i + 1;
    File.WriteAllText($"output/page_{n}.md", doc.ToMarkdown(i));
    Console.WriteLine($"Saved: output/page_{n}.md");

    File.WriteAllText($"output/page_{n}.html", doc.ToHtml(i));
    Console.WriteLine($"Saved: output/page_{n}.html");

    File.WriteAllText($"output/page_{n}.txt", doc.ExtractText(i));
    Console.WriteLine($"Saved: output/page_{n}.txt");
}

Console.WriteLine("Done. Files written to output/");
return 0;
