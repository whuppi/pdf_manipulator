// Create PDFs from Markdown, HTML, and plain text.
// Run: dotnet run

using PdfOxide.Core;

Console.WriteLine("Creating PDFs...");

// From Markdown
var markdown = @"# Project Report

## Summary

This document was generated from **Markdown** using pdf_oxide.

- Fast rendering
- Clean typography
- Cross-platform
";
var pdf = Pdf.FromMarkdown(markdown);
pdf.Save("from_markdown.pdf");
Console.WriteLine("Saved: from_markdown.pdf");

// From HTML
var html = @"<html><body>
<h1>Invoice #1234</h1>
<p>Generated from <em>HTML</em> using pdf_oxide.</p>
<table><tr><th>Item</th><th>Price</th></tr>
<tr><td>Widget</td><td>$9.99</td></tr></table>
</body></html>";
pdf = Pdf.FromHtml(html);
pdf.Save("from_html.pdf");
Console.WriteLine("Saved: from_html.pdf");

// From plain text
var text = "Hello, World!\n\nThis PDF was created from plain text using pdf_oxide.";
pdf = Pdf.FromText(text);
pdf.Save("from_text.pdf");
Console.WriteLine("Saved: from_text.pdf");

Console.WriteLine("Done. 3 PDFs created.");
