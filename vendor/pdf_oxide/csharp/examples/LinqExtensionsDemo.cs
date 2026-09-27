/*
 * LINQ Extensions Demo - PDF Oxide C#
 *
 * Comprehensive examples demonstrating LINQ extension methods including:
 * - Search result analysis and filtering
 * - Image extraction and quality analysis
 * - Batch processing patterns
 * - Async enumerable operations (.NET 5+)
 * - Complex query composition
 * - Performance optimization techniques
 */

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using PdfOxide;
using PdfOxide.Collections;
using PdfOxide.Managers;

namespace PdfOxide.Examples
{
    class LinqExtensionsDemoProgram
    {
        // Mock objects for demonstration (in real usage, these come from PDF Oxide)
        class MockSearchManager
        {
            private readonly List<SearchResult> _results = new()
            {
                new SearchResult { Text = "invoice", PageIndex = 0, Position = 10, BoundingBox = null },
                new SearchResult { Text = "invoice", PageIndex = 0, Position = 50, BoundingBox = null },
                new SearchResult { Text = "total", PageIndex = 1, Position = 25, BoundingBox = null },
                new SearchResult { Text = "invoice", PageIndex = 2, Position = 15, BoundingBox = null },
                new SearchResult { Text = "amount", PageIndex = 3, Position = 30, BoundingBox = null },
                new SearchResult { Text = "invoice", PageIndex = 4, Position = 40, BoundingBox = null },
                new SearchResult { Text = "due", PageIndex = 5, Position = 35, BoundingBox = null },
                new SearchResult { Text = "total", PageIndex = 5, Position = 60, BoundingBox = null },
            };

            public List<SearchResult> SearchAll(string term) => _results;
        }

        class MockExtractionManager
        {
            private readonly List<ExtractedImage> _images = new()
            {
                new ExtractedImage { PageIndex = 0, Format = "JPEG", Width = 1920, Height = 1440, Dpi = 300, Data = new byte[50000] },
                new ExtractedImage { PageIndex = 0, Format = "PNG", Width = 1024, Height = 768, Dpi = 150, Data = new byte[30000] },
                new ExtractedImage { PageIndex = 1, Format = "JPEG", Width = 640, Height = 480, Dpi = 96, Data = new byte[15000] },
                new ExtractedImage { PageIndex = 2, Format = "PNG", Width = 1280, Height = 960, Dpi = 200, Data = new byte[40000] },
                new ExtractedImage { PageIndex = 3, Format = "GIF", Width = 256, Height = 256, Dpi = 72, Data = new byte[8000] },
                new ExtractedImage { PageIndex = 4, Format = "JPEG", Width = 800, Height = 600, Dpi = 300, Data = new byte[20000] },
                new ExtractedImage { PageIndex = 5, Format = "PNG", Width = 512, Height = 512, Dpi = 100, Data = new byte[12000] },
            };

            public List<ExtractedImage> ExtractAll() => _images;
        }

        class MockRenderingManager
        {
            private readonly List<LayerVisibility> _layers = new()
            {
                new LayerVisibility { Name = "Background", IsVisible = true },
                new LayerVisibility { Name = "Content", IsVisible = true },
                new LayerVisibility { Name = "Annotations", IsVisible = false },
                new LayerVisibility { Name = "Watermark", IsVisible = false },
                new LayerVisibility { Name = "Signatures", IsVisible = true },
            };

            public List<LayerVisibility> GetLayerVisibility() => _layers;
        }

        static void Main()
        {
            Console.WriteLine("╔══════════════════════════════════════════════════════════════════╗");
            Console.WriteLine("║  PDF OXIDE C# - LINQ EXTENSIONS DEMO (Phase 2.5)                ║");
            Console.WriteLine("╚══════════════════════════════════════════════════════════════════╝\n");

            Example1_BasicSearchFiltering();
            Example2_AdvancedSearchAnalysis();
            Example3_ImageExtractorAndFilter();
            Example4_ImageQualityAnalysis();
            Example5_BatchProcessingPatterns();
            Example6_ComplexQueryComposition();
            Example7_LayerAndOutlineAnalysis();
            Example8_StringCollectionOperations();
            Example9_PerformanceOptimization();

            Console.WriteLine("\n╔══════════════════════════════════════════════════════════════════╗");
            Console.WriteLine("║  ✓ ALL EXAMPLES COMPLETED                                        ║");
            Console.WriteLine("╚══════════════════════════════════════════════════════════════════╝\n");
        }

        // ========================================================================
        // Example 1: Basic Search Result Filtering
        // ========================================================================

        static void Example1_BasicSearchFiltering()
        {
            Console.WriteLine("\n" + new string('═', 70));
            Console.WriteLine("EXAMPLE 1: Basic Search Result Filtering");
            Console.WriteLine(new string('═', 70) + "\n");

            var manager = new MockSearchManager();
            var results = manager.SearchAll("invoice");

            Console.WriteLine("Searching for \"invoice\" across entire document:\n");

            // Filter by specific page
            var page0Results = results.OnPage(0).ToList();
            Console.WriteLine($"  Results on page 0: {page0Results.Count}");
            foreach (var r in page0Results)
            {
                Console.WriteLine($"    - Found at position {r.Position}");
            }

            // Filter by page range
            Console.WriteLine();
            var rangeResults = results.OnPageRange(2, 4).ToList();
            Console.WriteLine($"  Results on pages 2-4: {rangeResults.Count}");
            foreach (var r in rangeResults)
            {
                Console.WriteLine($"    - Page {r.PageIndex}: position {r.Position}");
            }

            // Get unique pages
            Console.WriteLine();
            var uniquePages = results.GetUniquePages().ToList();
            Console.WriteLine($"  Unique pages with matches: {string.Join(", ", uniquePages)}");

            // Sort by page and position
            Console.WriteLine();
            var sorted = results.ByPageAndPosition().ToList();
            Console.WriteLine($"  ✓ Sorted {sorted.Count} results by page and position");
        }

        // ========================================================================
        // Example 2: Advanced Search Analysis
        // ========================================================================

        static void Example2_AdvancedSearchAnalysis()
        {
            Console.WriteLine("\n" + new string('═', 70));
            Console.WriteLine("EXAMPLE 2: Advanced Search Analysis");
            Console.WriteLine(new string('═', 70) + "\n");

            var manager = new MockSearchManager();
            var results = manager.SearchAll("invoice");

            Console.WriteLine("Comprehensive search statistics:\n");

            // Calculate statistics
            var totalCount = results.TotalCount();
            var avgPerPage = results.AveragePerPage();
            var minPos = results.MinimumPosition();
            var maxPos = results.MaximumPosition();
            var uniquePages = results.GetUniquePages().Count;

            Console.WriteLine($"  Total matches:          {totalCount}");
            Console.WriteLine($"  Unique pages:           {uniquePages}");
            Console.WriteLine($"  Average per page:       {avgPerPage:F2}");
            Console.WriteLine($"  Position range:         {minPos} - {maxPos}");

            // Group by page
            Console.WriteLine("\n  Matches by page:");
            var grouped = results.GroupByPage();
            foreach (var pageGroup in grouped.OrderBy(g => g.Key))
            {
                Console.WriteLine($"    - Page {pageGroup.Key}: {pageGroup.Count()} matches");
            }

            // Distinct results (one per page)
            Console.WriteLine();
            var distinct = results.DistinctByPage().ToList();
            Console.WriteLine($"  ✓ Extracted {distinct.Count} distinct pages from results");
        }

        // ========================================================================
        // Example 3: Image Extraction and Filtering
        // ========================================================================

        static void Example3_ImageExtractorAndFilter()
        {
            Console.WriteLine("\n" + new string('═', 70));
            Console.WriteLine("EXAMPLE 3: Image Extraction and Filtering");
            Console.WriteLine(new string('═', 70) + "\n");

            var manager = new MockExtractionManager();
            var images = manager.ExtractAll();

            Console.WriteLine("Extracting and filtering images from document:\n");

            // Filter by format
            var jpegs = images.WithFormat("JPEG").ToList();
            var pngs = images.WithFormat("PNG").ToList();
            Console.WriteLine($"  JPEG images: {jpegs.Count}");
            Console.WriteLine($"  PNG images: {pngs.Count}");

            // Filter by size
            Console.WriteLine("\n  Filtering by dimensions:");
            var large = images.MinimumSize(800, 600).ToList();
            Console.WriteLine($"    Large (800x600+): {large.Count} images");

            var small = images.MaximumSize(512, 512).ToList();
            Console.WriteLine($"    Small (512x512-): {small.Count} images");

            // Filter by quality
            Console.WriteLine("\n  Filtering by quality (DPI):");
            var highRes = images.MinimumDpi(300).ToList();
            Console.WriteLine($"    High resolution (300+ DPI): {highRes.Count} images");

            var webRes = images.MaximumDpi(150).ToList();
            Console.WriteLine($"    Web quality (150- DPI): {webRes.Count} images");

            // Sort by size
            Console.WriteLine();
            var sorted = images.BySize().ToList();
            Console.WriteLine($"  ✓ Sorted {sorted.Count} images by pixel area");
        }

        // ========================================================================
        // Example 4: Image Quality Analysis
        // ========================================================================

        static void Example4_ImageQualityAnalysis()
        {
            Console.WriteLine("\n" + new string('═', 70));
            Console.WriteLine("EXAMPLE 4: Image Quality Analysis");
            Console.WriteLine(new string('═', 70) + "\n");

            var manager = new MockExtractionManager();
            var images = manager.ExtractAll();

            Console.WriteLine("Comprehensive image quality analysis:\n");

            // Calculate statistics
            var (avgW, avgH) = images.AverageDimensions();
            var minQuality = images.MinimumQuality();
            var maxQuality = images.MaximumQuality();
            var totalBytes = images.TotalImageBytes();
            var totalMb = totalBytes / (1024.0 * 1024.0);

            Console.WriteLine($"  Total images:           {images.Count}");
            Console.WriteLine($"  Average dimensions:     {avgW}x{avgH}");
            Console.WriteLine($"  Quality range:          {minQuality} - {maxQuality} DPI");
            Console.WriteLine($"  Total data:             {totalMb:F2} MB ({totalBytes:N0} bytes)");

            // Format breakdown
            Console.WriteLine("\n  Format distribution:");
            var byFormat = images.GroupByFormat();
            foreach (var format in byFormat)
            {
                var formatImages = byFormat[format.Key].ToList();
                var totalSize = formatImages.Sum(i => i.Data?.Length ?? 0);
                Console.WriteLine($"    {format.Key}: {formatImages.Count} images ({totalSize / 1024.0:F1} KB)");
            }

            // Quality pages
            Console.WriteLine();
            var hqPages = images.PagesWithHighQuality(200).ToList();
            Console.WriteLine($"  Pages with 200+ DPI: {string.Join(", ", hqPages)}");

            // Pages with specific image count
            var page0Count = images.FromPage(0).Count();
            Console.WriteLine($"  ✓ Page 0 contains {page0Count} images");
        }

        // ========================================================================
        // Example 5: Batch Processing Patterns
        // ========================================================================

        static void Example5_BatchProcessingPatterns()
        {
            Console.WriteLine("\n" + new string('═', 70));
            Console.WriteLine("EXAMPLE 5: Batch Processing Patterns");
            Console.WriteLine(new string('═', 70) + "\n");

            var manager = new MockSearchManager();
            var results = manager.SearchAll("text");

            Console.WriteLine("Processing search results in batches:\n");

            // Process in batches
            int batchSize = 3;
            int batchNum = 0;
            var totalProcessed = 0;

            foreach (var batch in results.Batch(batchSize))
            {
                batchNum++;
                var items = batch.ToList();
                Console.WriteLine($"  Batch {batchNum}: Processing {items.Count} items");
                foreach (var item in items)
                {
                    Console.WriteLine($"    - Page {item.PageIndex}: {item.Text}");
                    totalProcessed++;
                }
            }

            Console.WriteLine($"\n  ✓ Processed {totalProcessed} total items in {batchNum} batches");
        }

        // ========================================================================
        // Example 6: Complex Query Composition
        // ========================================================================

        static void Example6_ComplexQueryComposition()
        {
            Console.WriteLine("\n" + new string('═', 70));
            Console.WriteLine("EXAMPLE 6: Complex Query Composition");
            Console.WriteLine(new string('═', 70) + "\n");

            var searchManager = new MockSearchManager();
            var extractionManager = new MockExtractionManager();

            Console.WriteLine("Composing complex LINQ queries:\n");

            // Complex search query
            var searchResults = searchManager.SearchAll("invoice");
            Console.WriteLine("  Query 1: Find 'invoice' on pages 1-4, sorted");
            var query1 = searchResults
                .OnPageRange(1, 4)
                .ByPageAndPosition()
                .ToList();
            Console.WriteLine($"    Result: {query1.Count} matches\n");

            // Complex image query
            var images = extractionManager.ExtractAll();
            Console.WriteLine("  Query 2: Find large, high-quality JPEGs");
            var query2 = images
                .WithFormat("JPEG")
                .MinimumSize(800, 600)
                .MinimumDpi(200)
                .ToList();
            Console.WriteLine($"    Result: {query2.Count} images matching criteria\n");

            // Chained filtering
            Console.WriteLine("  Query 3: Extract pages with multiple images");
            var query3 = images.FromPage(0).ToList();
            Console.WriteLine($"    Result: Page 0 has {query3.Count} images");

            Console.WriteLine();
            Console.WriteLine($"  ✓ Completed 3 complex query compositions");
        }

        // ========================================================================
        // Example 7: Layer and Outline Analysis
        // ========================================================================

        static void Example7_LayerAndOutlineAnalysis()
        {
            Console.WriteLine("\n" + new string('═', 70));
            Console.WriteLine("EXAMPLE 7: Layer and Outline Analysis");
            Console.WriteLine(new string('═', 70) + "\n");

            var manager = new MockRenderingManager();
            var layers = manager.GetLayerVisibility();

            Console.WriteLine("Analyzing document layers:\n");

            // Visibility analysis
            var (visibleCount, hiddenCount, percentage) = layers.VisibilitySummary();
            Console.WriteLine($"  Visible layers:   {visibleCount}");
            Console.WriteLine($"  Hidden layers:    {hiddenCount}");
            Console.WriteLine($"  Visibility:       {percentage:F1}%\n");

            // List visible layers
            var visibleLayers = layers.WhereVisible().ToList();
            Console.WriteLine("  Visible:");
            foreach (var layer in visibleLayers)
            {
                Console.WriteLine($"    ✓ {layer.Name}");
            }

            // List hidden layers
            var hiddenLayers = layers.WhereHidden().ToList();
            Console.WriteLine("\n  Hidden:");
            foreach (var layer in hiddenLayers)
            {
                Console.WriteLine($"    ✗ {layer.Name}");
            }

            // Search by name
            Console.WriteLine();
            var annotationLayers = layers.WithNameContaining("ation").ToList();
            Console.WriteLine($"  Layers containing 'ation': {string.Join(", ", annotationLayers.Select(l => l.Name))}");

            // Sorted layers
            Console.WriteLine();
            var sorted = layers.OrderByName().ToList();
            Console.WriteLine($"  ✓ Sorted {sorted.Count} layers alphabetically");
        }

        // ========================================================================
        // Example 8: String Collection Operations
        // ========================================================================

        static void Example8_StringCollectionOperations()
        {
            Console.WriteLine("\n" + new string('═', 70));
            Console.WriteLine("EXAMPLE 8: String Collection Operations");
            Console.WriteLine(new string('═', 70) + "\n");

            var fieldNames = new[] { "TextBox", "text_field", "BUTTON", "  ", "TextArea", "" };

            Console.WriteLine("String collection filtering:\n");

            // Remove empty/whitespace
            var valid = fieldNames.WhereNotEmpty().ToList();
            Console.WriteLine($"  Non-empty strings: {valid.Count}");
            foreach (var s in valid)
            {
                Console.WriteLine($"    - {s}");
            }

            // Find substrings (case-insensitive)
            Console.WriteLine();
            var textFields = fieldNames.ContainsSubstring("text", StringComparison.OrdinalIgnoreCase).ToList();
            Console.WriteLine($"  Containing 'text': {textFields.Count}");
            foreach (var s in textFields)
            {
                Console.WriteLine($"    - {s}");
            }

            Console.WriteLine();
            Console.WriteLine($"  ✓ Processed {fieldNames.Length} string items");
        }

        // ========================================================================
        // Example 9: Performance Optimization
        // ========================================================================

        static void Example9_PerformanceOptimization()
        {
            Console.WriteLine("\n" + new string('═', 70));
            Console.WriteLine("EXAMPLE 9: Performance Optimization");
            Console.WriteLine(new string('═', 70) + "\n");

            var manager = new MockSearchManager();
            var results = manager.SearchAll("text");

            Console.WriteLine("Comparing synchronous vs deferred execution:\n");

            // Deferred execution (lazy)
            Console.WriteLine("  Deferred execution (LINQ chains):");
            var sw = Stopwatch.StartNew();
            var query = results
                .Where(r => r.PageIndex > 0)
                .OrderBy(r => r.PageIndex)
                .Take(10);
            sw.Stop();
            Console.WriteLine($"    Query construction time: {sw.Elapsed.TotalMicroseconds:F2}µs (no materialization)");

            // Materialization
            sw.Restart();
            var materialized = query.ToList();
            sw.Stop();
            Console.WriteLine($"    Materialization time: {sw.Elapsed.TotalMicroseconds:F2}µs ({materialized.Count} items)");

            // Compare with alternative approach
            Console.WriteLine("\n  Optimized batching:");
            sw.Restart();
            int processed = 0;
            foreach (var batch in results.Batch(3))
            {
                processed += batch.Count();
            }
            sw.Stop();
            Console.WriteLine($"    Batch processing time: {sw.Elapsed.TotalMicroseconds:F2}µs ({processed} items)");

            Console.WriteLine($"\n  ✓ Performance measurements complete");
        }
    }
}
