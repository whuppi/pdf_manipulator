# C# Extension Methods Guide

PdfOxide provides 100+ extension methods for convenient PDF operations. All extension methods are in the `PdfOxide.Extensions` namespace.

## Document Extension Methods (PdfDocumentExtensions)

Shortcuts to create managers from documents:

```csharp
var doc = PdfDocument.Open("document.pdf");

doc.Pages()         // PageManager
doc.Search()        // SearchManager
doc.Extract()       // ExtractionManager
doc.Forms()         // FormManager
doc.Security()      // SecurityManager
doc.Outlines()      // OutlineManager
doc.Layers()        // LayerManager
doc.Metadata()      // MetadataManager
```

### Text Operations

```csharp
// Extract all text as single string
string allText = doc.ExtractAllText();

// Extract text from specific page
string pageText = doc.ExtractPageText(0);

// Search for text
var results = doc.SearchDocument("keyword");

// Check if text exists
bool found = doc.ContainsText("specific text");
```

### Content Operations

```csharp
// Extract all images
var images = doc.ExtractAllImages();

// Get form field names
var fields = doc.GetFormFieldNames();

// Check for forms
bool hasForms = doc.HasForms();

// Get bookmarks
var bookmarks = doc.GetOutlines();

// Check for bookmarks
bool hasOutlines = doc.HasOutlines();

// Get layer names
var layers = doc.GetLayerNames();

// Check for layers
bool hasLayers = doc.HasLayers();
```

### Metadata Operations

```csharp
// Get document title
string title = doc.GetTitle();

// Get document author
string author = doc.GetAuthor();

// Get metadata manager for more details
var metadata = doc.Metadata();
```

### Security Operations

```csharp
// Check encryption status
bool isEncrypted = doc.IsEncrypted();

// Check permissions
bool canPrint = doc.CanPrint();
bool canCopy = doc.CanCopy();
bool canModify = doc.CanModify();
bool canFillForms = doc.CanFillForms();
bool canAnnotate = doc.CanAnnotate();

// Check view-only status
bool isViewOnly = doc.IsViewOnly();

// Get security manager for detailed info
var security = doc.Security();
```

### Page Information

```csharp
// Check if empty
bool isEmpty = doc.IsEmpty();

// Check for multiple pages
bool hasMultiple = doc.HasMultiplePages();

// Get page information
var allPages = doc.GetPageInfo();

// Get first page index
int first = doc.GetFirstPageIndex();

// Get last page index
int last = doc.GetLastPageIndex();

// Get middle page index
int middle = doc.GetMiddlePageIndex();

// Validate page index
bool isValid = doc.IsValidPage(5);

// Access PageManager
var pages = doc.Pages();
```

## Page Extension Methods (PdfPageExtensions)

### Manager Shortcuts

```csharp
var page = doc.GetPage(0);

page.Annotations()  // AnnotationManager for this page
page.Content()      // ContentManager for this page
```

### Content Information

```csharp
// Get dimension summary string
string dims = page.GetDimensions();

// Check if page has content
bool hasContent = page.HasContent();

// Check if page is blank
bool isBlank = page.IsBlank();

// Get content types
var types = page.GetContentTypes();  // Returns StringCollection

// Get content summary
string summary = page.GetContentSummary();

// Get complexity score (0-100)
int complexity = page.GetComplexity();
```

### Content Detection

```csharp
// Check for forms
bool likelyHasForms = page.LikelyHasForms();

// Check for tables
bool likelyHasTables = page.LikelyHasTables();

// Check for images
bool likelyHasImages = page.LikelyHasImages();
```

### Annotation Operations

```csharp
// Get annotation count
int count = page.GetAnnotationCount();

// Check if has annotations
bool hasAnnotations = page.HasAnnotations();

// Get annotation types
var types = page.GetAnnotationTypes();  // Returns StringCollection
```

### Dimension Methods

```csharp
// Get width in points
float width = page.GetWidth();

// Get height in points
float height = page.GetHeight();

// Check orientation
bool isPortrait = page.IsPortrait();
bool isLandscape = page.IsLandscape();

// Get aspect ratio
float ratio = page.GetAspectRatio();

// Get area
float area = page.GetArea();

// Get standard size name
string size = page.GetStandardSize();  // "Letter", "A4", etc.

// Compare with another page
bool sameSzie = page.SameSizeAs(otherPage);
```

## LINQ Collection Extension Methods (PdfLinqExtensions)

### String Extensions

```csharp
IEnumerable<string> strings = new[] { "Hello", "Help", "Goodbye" };

// Case-insensitive substring search
var results = strings.ContainsSubstring("hel");

// Filter non-empty/whitespace strings
var nonEmpty = strings.WhereNotEmpty();
```

### Search Result Extensions

```csharp
IEnumerable<SearchResult> results = searchCollection;

// Filter by page
var page5 = results.OnPage(5);

// Filter by page range
var pages5to10 = results.OnPageRange(5, 10);

// Get unique pages
var uniquePages = results.GetUniquePages();

// Sort by page and position
var sorted = results.ByPageAndPosition();

// Group by page
var grouped = results.GroupByPage();
```

### Image Extensions

```csharp
IEnumerable<ExtractedImage> images = imageCollection;

// Filter by format
var jpegs = images.WithFormat("JPEG");

// Filter by page
var page3Images = images.FromPage(3);

// Filter by size
var large = images.MinimumSize(1000, 1000);
var small = images.MaximumSize(500, 500);

// Filter by DPI
var highRes = images.MinimumDpi(300);
var lowRes = images.MaximumDpi(150);

// Sort by size (largest first)
var bySize = images.BySize();

// Group by page
var byPage = images.GroupByPage();

// Group by format
var byFormat = images.GroupByFormat();

// Calculate total size
long bytes = images.TotalSizeBytes();
double mb = images.TotalSizeMb();
```

### Page Extension

```csharp
IEnumerable<PageInfo> pages = pageCollection;

// Get page range
var range = pages.InRange(5, 10);

// Get every nth page
var everyOther = pages.EveryNthPage(2);
```

### Layer Extensions

```csharp
IEnumerable<LayerVisibility> layers = layerCollection;

// Filter visible
var visible = layers.WhereVisible();

// Filter hidden
var hidden = layers.WhereHidden();

// Filter by name
var matching = layers.WithNameContaining("layer");

// Sort
var sorted = layers.OrderByName();
var sortedByVis = layers.OrderByVisibility();
```

### Outline Extensions

```csharp
IEnumerable<OutlineItem> outlines = outlineCollection;

// Filter by page
var page10 = outlines.TargetingPage(10);

// Filter by page range
var pages5to15 = outlines.TargetingPageRange(5, 15);

// Filter with children
var parents = outlines.WhereHasChildren();

// Filter leaf nodes
var leaves = outlines.WhereLeaf();

// Filter by expansion state
var expanded = outlines.WhereExpanded();
var collapsed = outlines.WhereCollapsed();

// Sort
var byPage = outlines.OrderByPage();
var byTitle = outlines.OrderByTitle();

// Group
var grouped = outlines.GroupByPage();
```

## Fluent Query Extensions (FluentQueryExtensions)

### Custom Filtering

```csharp
var images = doc.ExtractAllImages();

// Custom where clause
var filtered = images.Where(img => img.Dpi > 200 && img.Width > 500);

// Projection (Select)
var sizes = images.Select(img => new { img.Width, img.Height });
```

### Aggregates

```csharp
var results = doc.Search().SearchAll("text");

// Check if any match condition
bool hasLarge = results.Any(r => r.Text.Length > 100);

// Count matching
int largeCount = results.Count(r => r.Text.Length > 100);
```

### Pagination

```csharp
var images = doc.ExtractAllImages();

// Skip elements
var skipped = images.Skip(5);

// Take elements
var first10 = images.Take(10);

// Combined
var page2 = images.Skip(10).Take(10);
```

### Statistics

```csharp
var images = doc.ExtractAllImages()
    .WithFormat("PNG")
    .MinimumDpi(150);

var stats = images.GetStatistics();
Console.WriteLine($"Count: {stats.TotalCount}");
Console.WriteLine($"Size: {stats.TotalSizeMb:F2}MB");
Console.WriteLine($"Avg DPI: {stats.AverageDpi:F0}");
Console.WriteLine($"Formats: {stats.UniqueForms}");
Console.WriteLine($"Pages: {stats.UniquePages}");

// Search result statistics
var searchStats = doc.Search()
    .SearchAll("important")
    .GetStatistics();
Console.WriteLine($"Results: {searchStats.TotalResults}");
Console.WriteLine($"Pages: {searchStats.UniquePages}");
```

## Real-World Examples

### Analyze Document Content

```csharp
using (var doc = PdfDocument.Open("report.pdf"))
{
    // Overview
    Console.WriteLine($"Pages: {doc.PageCount}");
    Console.WriteLine($"Title: {doc.GetTitle()}");
    Console.WriteLine($"Author: {doc.GetAuthor()}");
    Console.WriteLine($"Encrypted: {doc.IsEncrypted()}");
    Console.WriteLine($"Can Print: {doc.CanPrint()}");

    // Find specific content
    bool hasKeyword = doc.ContainsText("critical");
    var images = doc.ExtractAllImages()
        .GetStatistics();
    Console.WriteLine($"Images: {images}");

    // Analyze pages
    var pages = doc.GetPageInfo();
    var firstPage = pages.FirstPage;
    var lastPage = pages.LastPage;
    Console.WriteLine($"Range: {firstPage.Number}-{lastPage.Number}");
}
```

### Extract and Process Images

```csharp
var images = doc.ExtractAllImages()
    .WithFormat("JPEG")
    .MinimumDpi(300)
    .SortBySize();

foreach (var image in images.Take(5))
{
    Console.WriteLine($"Page {image.PageIndex}: {image.Width}x{image.Height}@{image.Dpi}DPI");
    File.WriteAllBytes($"image_{image.PageIndex}.jpg", image.Data);
}
```

### Search and Analyze

```csharp
var results = doc.SearchDocument("important")
    .Where(r => r.Text.Length > 10)
    .GetStatistics();

Console.WriteLine($"Found '{results}}'");

var byPage = doc.SearchDocument("term")
    .GroupByPage();
foreach (var pageGroup in byPage)
{
    Console.WriteLine($"Page {pageGroup.Key}: {pageGroup.Count()} occurrences");
}
```

### Analyze Structure

```csharp
if (doc.HasOutlines())
{
    var outline = doc.GetOutlines()
        .Flattened()
        .Where(o => o.TargetPageIndex > 0);

    foreach (var item in outline)
    {
        Console.WriteLine($"{item.Title} → Page {item.TargetPageIndex + 1}");
    }
}

if (doc.HasLayers())
{
    var layers = doc.Layers()
        .GetAllLayerVisibility()
        .Visible()
        .OrderByName();

    foreach (var layer in layers)
    {
        Console.WriteLine($"✓ {layer.Name}");
    }
}
```

## See Also

- [LINQ Support Guide](LINQ_SUPPORT.md)
- [Collections Reference](COLLECTIONS_REFERENCE.md)
- [API Reference](API_REFERENCE.md)
- [Migration Guide](MIGRATION_GUIDE.md)
