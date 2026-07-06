// StreamingTable with rowspan and batchSize
// Run: dotnet run

using System;
using System.IO;
using PdfOxide.Core;

const string OutDir = "output";
Directory.CreateDirectory(OutDir);

Console.WriteLine("Building streaming table with rowspan...");

var columns = new[]
{
    new Column("Category", 120),
    new Column("Item",     160),
    new Column("Notes",    150, Alignment.Right),
};

using var builder = DocumentBuilder.Create();
builder.Title("StreamingTable Demo");
var page = builder.LetterPage();
page.Font("Helvetica", 10).At(72, 700).Heading(1, "Product Catalogue").At(72, 660);

using (var tbl = page.StreamingTable(columns, repeatHeader: true, maxRowspan: 2, batchSize: 2))
{
    tbl.AddRowSpan(("Fruits", 2), ("Apple", 1),   ("crisp",  1));
    tbl.AddRowSpan(("",       1), ("Banana", 1),  ("sweet",  1));
    tbl.AddRowSpan(("Vegetables", 1), ("Carrot", 1), ("earthy", 1));
    Console.WriteLine($"  batch_count={tbl.BatchCount}, pending={tbl.PendingRowCount}");
    tbl.Build().Done();
}

var path = Path.Combine(OutDir, "streaming_table_rowspan.pdf");
builder.Save(path);
Console.WriteLine($"Written: {path}");
