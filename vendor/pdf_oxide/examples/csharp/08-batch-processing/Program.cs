// Process multiple PDFs concurrently using Task.WhenAll.
// Run: dotnet run --project csharp/ -- file1.pdf file2.pdf ...

using System.Diagnostics;
using PdfOxide.Core;

if (args.Length < 1)
{
    Console.Error.WriteLine("Usage: dotnet run -- <file1.pdf> <file2.pdf> ...");
    return 1;
}

Console.WriteLine($"Processing {args.Length} PDFs concurrently...");
var sw = Stopwatch.StartNew();

var tasks = args.Select(path => Task.Run(() =>
{
    try
    {
        using var doc = PdfDocument.Open(path);
        int pages = doc.PageCount;
        int totalChars = 0;
        for (int p = 0; p < pages; p++)
        {
            totalChars += doc.ExtractText(p).Length;
        }
        return (path, pages, totalChars, error: (string?)null);
    }
    catch (Exception ex)
    {
        return (path, pages: 0, totalChars: 0, error: ex.Message);
    }
})).ToArray();

var results = await Task.WhenAll(tasks);
foreach (var r in results)
{
    if (r.error != null)
        Console.WriteLine($"[{r.path}]\tERROR: {r.error}");
    else
        Console.WriteLine($"[{r.path}]\tpages={r.pages}\tchars={r.totalChars}");
}

sw.Stop();
Console.WriteLine($"\nDone: {args.Length} files processed in {sw.Elapsed.TotalSeconds:F2}s");
return 0;
