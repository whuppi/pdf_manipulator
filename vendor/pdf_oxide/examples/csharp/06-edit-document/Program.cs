// Open a PDF, modify metadata, delete a page, and save.
// Run: dotnet run --project csharp/ -- input.pdf output.pdf

using PdfOxide.Core;

if (args.Length < 2)
{
    Console.Error.WriteLine("Usage: dotnet run -- <input.pdf> <output.pdf>");
    return 1;
}

var input = args[0];
var output = args[1];

using var editor = DocumentEditor.Open(input);
Console.WriteLine($"Opened: {input}");

editor.Title = "Edited Document";
Console.WriteLine("Set title: \"Edited Document\"");

editor.Author = "pdf_oxide";
Console.WriteLine("Set author: \"pdf_oxide\"");

if (editor.PageCount > 1)
{
    editor.DeletePage(1); // 0-indexed, deletes page 2
    Console.WriteLine("Deleted page 2");
}
else
{
    Console.WriteLine("(skipped delete — single-page document)");
}

editor.Save(output);
Console.WriteLine($"Saved: {output}");

return 0;
