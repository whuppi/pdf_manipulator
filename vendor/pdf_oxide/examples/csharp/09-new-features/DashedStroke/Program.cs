// Dashed stroke lines and rectangles
// Run: dotnet run

using System;
using System.IO;
using PdfOxide.Core;

const string OutDir = "output";
Directory.CreateDirectory(OutDir);

Console.WriteLine("Building dashed stroke demo...");

using var builder = DocumentBuilder.Create();
builder.Title("Dashed Stroke Demo");
var page = builder.LetterPage();

page.Font("Helvetica", 12f)
    .At(72f, 720f).Heading(1, "Dashed Stroke Demo")
    .At(72f, 680f).Text("Rectangles and lines drawn with configurable dash patterns.");

// Dashed rectangle — [5 on, 3 off] pattern, blue border
page.StrokeRectDashed(72f, 580f, 300f, 80f,
    dash: new float[] { 5f, 3f }, phase: 0f,
    width: 2f, r: 0f, g: 0.2f, b: 0.8f);

// Dashed line — [8 on, 4 off] pattern, red
page.StrokeLineDashed(72f, 550f, 372f, 550f,
    dash: new float[] { 8f, 4f }, phase: 0f,
    width: 1.5f, r: 0.8f, g: 0f, b: 0f);

// Fine dotted rectangle — [2 on, 2 off] with phase offset, green
page.StrokeRectDashed(72f, 460f, 200f, 60f,
    dash: new float[] { 2f, 2f }, phase: 1f,
    width: 1f, r: 0f, g: 0.6f, b: 0f);

page.Done();

var path = Path.Combine(OutDir, "dashed_stroke.pdf");
builder.Save(path);
Console.WriteLine($"Written: {path}");
