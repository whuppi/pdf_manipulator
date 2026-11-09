# C# LINQ Support Guide

The PdfOxide C# bindings provide comprehensive LINQ support for querying and analyzing PDF collections. All collection types implement `IReadOnlyList<T>` and `IEnumerable<T>`, enabling full LINQ compatibility.

## Supported Collections

### Core Collection Types

- **`StringCollection`** - Collections of strings (field names, layer names, content types)
- **`SearchResultCollection`** - Search results with page/position information
- **`ExtractedImageCollection`** - Extracted images with format and resolution info
- **`PageInfoCollection`** - Page metadata and information
- **`OutlineItemCollection`** - Document bookmarks/outlines (hierarchical)
- **`LayerVisibilityCollection`** - Optional content groups with visibility state

All collections inherit from `PdfCollectionBase<T>` which provides:
- Standard LINQ methods (`Where`, `Select`, `OrderBy`, `GroupBy`, etc.)
- Convenience properties (`IsEmpty`, `HasItems`, `FirstOrDefault`, `LastOrDefault`)
- Collection conversion (`ToList()`, `ToArray()`, `Copy()`)

## LINQ Examples

### String Collections

```csharp
var doc = PdfDocument.Open("document.pdf");

// Get all non-empty field names
var fieldNames = doc.Forms()
    .GetAllFieldNames()
    .NonEmpty()
    .OrderAlphabetically();

// Find fields starting with "contact"
var contactFields = doc.Forms()
    .GetAllFieldNames()
    .Where(name => name.StartsWith("contact", StringComparison.OrdinalIgnoreCase));

// Get unique field names
var uniqueNames = doc.Forms()
    .GetAllFieldNames()
    .DistinctIgnoreCase();

// Join field names with commas
var fieldList = doc.Forms()
    .GetAllFieldNames()
    .Join(", ");
```

### Search Results

```csharp
// Find all results on a specific page
var pageResults = doc.Search()
    .SearchAll("keyword")
    .OnPage(5);

// Find results in a page range
var rangeResults = doc.Search()
    .SearchAll("term")
    .OnPages(10, 20);

// Sort by page and position
var sorted = doc.Search()
    .SearchAll("text")
    .SortByPageAndPosition();

// Get results by unique pages
var pagesWithResults = doc.Search()
    .SearchAll("keyword")
    .UniquePages();

// Complex query with grouping
var grouped = doc.Search()
    .SearchAll("text")
    .ByPageAndPosition()
    .Cast<SearchResult>()
    .GroupBy(r => r.PageIndex)
    .OrderBy(g => g.Key);
```

### Extracted Images

```csharp
// Get high-resolution JPEG images
var highResJpegs = doc.ExtractAllImages()
    .WithFormat("JPEG")
    .MinimumDpi(300);

// Find large images
var largeImages = doc.ExtractAllImages()
    .LargerThan(1000, 1000)
    .SortBySize();

// Get images by page
var imagesPerPage = doc.ExtractAllImages()
    .ImagesPerPage();

// Calculate total size
var totalSizeMb = doc.ExtractAllImages().TotalSizeMb();

// Get statistics
var stats = doc.ExtractAllImages()
    .WithFormat("PNG")
    .GetStatistics();
```

### Page Information

```csharp
// Get pages in a range
var middlePages = doc.GetPageInfo()
    .Range(doc.GetPageCount() / 4, (doc.GetPageCount() * 3) / 4);

// Get every other page
var evenPages = doc.GetPageInfo()
    .EveryNthPage(2);

// Get first 5 pages
var firstFive = doc.GetPageInfo()
    .FirstPages(5);

// Get last 3 pages
var lastThree = doc.GetPageInfo()
    .LastPages(3);

// Check if page exists
bool hasPage5 = doc.GetPageInfo().HasPage(5);
```

### Outlines/Bookmarks

```csharp
var outlines = doc.GetOutlines();

// Find outline by title
var chapter = outlines.FindByTitle("Chapter 1");

// Get all expanded items
var expanded = outlines.Expanded();

// Get leaf items (no children)
var leaves = outlines.Leaves();

// Get items targeting specific page
var pageBookmarks = outlines.TargetingPage(10);

// Flatten and sort by page
var sorted = outlines
    .Flattened()
    .SortByPage();

// Get hierarchy string
var hierarchy = outlines.GetHierarchyString();
```

### Layer/OCG Collections

```csharp
var layers = doc.Layers().GetAllLayerVisibility();

// Get visible layers
var visible = layers.Visible().OrderByName();

// Find hidden layers
var hidden = layers.Hidden();

// Check if layer exists
bool hasLayer = layers.ContainsLayer("Layer Name");

// Get all unique pages with layers
var layerPages = layers.UniquePages();

// Get summary
var summary = layers.GetSummary();
```

## PDF-Specific Extension Methods

### Document Extensions (PdfDocumentExtensions)

```csharp
var doc = PdfDocument.Open("document.pdf");

// Manager shortcuts
var pages = doc.Pages();              // PageManager
var search = doc.Search();            // SearchManager
var extract = doc.Extract();          // ExtractionManager
var forms = doc.Forms();              // FormManager
var security = doc.Security();        // SecurityManager

// Direct operations
string allText = doc.ExtractAllText();
string pageText = doc.ExtractPageText(0);
var results = doc.SearchDocument("keyword");
bool hasKeyword = doc.ContainsText("keyword");
var images = doc.ExtractAllImages();

// Page information
int pageCount = doc.PageCount;
bool isEmpty = doc.IsEmpty();
bool hasMultiple = doc.HasMultiplePages();
var pageInfo = doc.GetPageInfo();

// Security
bool isEncrypted = doc.IsEncrypted();
bool canPrint = doc.CanPrint();
bool canCopy = doc.CanCopy();
bool isViewOnly = doc.IsViewOnly();
```

### Page Extensions (PdfPageExtensions)

```csharp
var page = doc.GetPage(0);

// Manager shortcuts
var annotations = page.Annotations();  // AnnotationManager
var content = page.Content();          // ContentManager

// Page information
string dimensions = page.GetDimensions();
bool hasContent = page.HasContent();
bool isBlank = page.IsBlank();
var contentTypes = page.GetContentTypes();

// Orientation
bool isPortrait = page.IsPortrait();
bool isLandscape = page.IsLandscape();
float aspectRatio = page.GetAspectRatio();
float area = page.GetArea();

// Standard size detection
string sizeType = page.GetStandardSize(); // "Letter", "A4", etc.

// Content analysis
bool hasForms = page.LikelyHasForms();
bool hasTables = page.LikelyHasTables();
bool hasImages = page.LikelyHasImages();
int complexity = page.GetComplexity();
```

## Combining LINQ with Extension Methods

### Complex Queries

```csharp
// Extract and analyze images with fluent API
var imageReport = doc.ExtractAllImages()
    .WithFormat("JPEG")
    .MinimumDpi(150)
    .SortBySize()
    .Take(10)
    .GetStatistics();

Console.WriteLine($"Top 10 JPEGs: {imageReport}");

// Find all text on specific pages
var targetPages = doc.GetPageInfo()
    .Range(5, 20)
    .Select(p => p.PageIndex);

var pageTexts = doc.ExtractAllText();

// Complex search with statistics
var searchStats = doc.Search()
    .SearchAll("important")
    .Where(r => r.Text.Length > 10)
    .GetStatistics();

// Analyze form fields and values
var formFieldInfo = doc.Forms()
    .GetAllFieldNames()
    .Where(name => !name.StartsWith("_"))
    .OrderAlphabetically()
    .ToList();

// Layer analysis
var layerSummary = doc.Layers()
    .GetAllLayerVisibility()
    .GroupByVisibility()
    .AsEnumerable();
```

## Performance Considerations

1. **Lazy Evaluation**: LINQ methods use deferred execution where possible
2. **Collection Caching**: For large collections, cache results when querying multiple times:
   ```csharp
   var images = doc.ExtractAllImages().ToList(); // Cache once
   var largeImages = images.Where(i => i.Width > 1000);
   var jpegs = images.Where(i => i.Format == "JPEG");
   ```

3. **Avoid Repeated Calls**: Don't call manager methods repeatedly:
   ```csharp
   // BAD - Creates multiple managers
   for (int i = 0; i < doc.Extract().ExtractAllImages().Count; i++) { }

   // GOOD - Reuse collection
   var images = doc.ExtractAllImages();
   for (int i = 0; i < images.Count; i++) { }
   ```

4. **Use Specialized Methods**: When available, use specialized collection methods instead of LINQ:
   ```csharp
   // More efficient
   int jpegs = images.UniqueFormats()["JPEG"];

   // vs
   int jpegs = images.Where(i => i.Format == "JPEG").Count();
   ```

## Type Safety

All collections are strongly typed with compile-time checking:

```csharp
// Compile-time error - wrong type
var results = doc.Forms().GetAllFieldNames();
var images = results.Where(i => i.Width > 100); // ERROR: strings don't have Width

// Correct - type checked
var images = doc.ExtractAllImages();
var filtered = images.Where(i => i.Width > 100); // OK
```

## See Also

- [Extension Methods Guide](EXTENSIONS_GUIDE.md)
- [Collections Reference](COLLECTIONS_REFERENCE.md)
- [API Reference](API_REFERENCE.md)
