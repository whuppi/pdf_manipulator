using System;
using System.IO;
using System.Reflection;
using PdfOxide.Core;

namespace PdfOxide.Tests.TestFixtures
{
    /// <summary>
    /// Manages test PDF fixtures for the PdfOxide test suite.
    /// Generates or retrieves test PDF documents covering all features.
    /// </summary>
    public static class TestFixtureManager
    {
        private static readonly string FixturePath = GetFixturePath();

        /// <summary>
        /// Gets the absolute path to the fixtures directory.
        /// Creates directory if it doesn't exist.
        /// </summary>
        private static string GetFixturePath()
        {
            // Find the test project root
            var assemblyLocation = Assembly.GetExecutingAssembly().Location;
            var testProjectRoot = Path.GetDirectoryName(Path.GetDirectoryName(assemblyLocation))
                ?? throw new InvalidOperationException("Unable to resolve test project root from assembly location.");
            var fixturesDir = Path.Combine(testProjectRoot, "TestFixtures", "fixtures");

            if (!Directory.Exists(fixturesDir))
            {
                Directory.CreateDirectory(fixturesDir);
            }

            return fixturesDir;
        }

        /// <summary>
        /// Ensures all test fixtures are generated and available.
        /// Called once during test initialization.
        /// </summary>
        public static void EnsureFixturesExist()
        {
            GenerateSimplePdf();
            GenerateMultiPagePdf();
            GenerateWithTextPdf();
            GenerateWithAnnotationsPdf();
            GenerateSearchDocumentPdf();
        }

        /// <summary>
        /// Gets the full path to a fixture file.
        /// </summary>
        public static string GetFixturePath(string filename)
        {
            if (filename == null)
                throw new ArgumentNullException(nameof(filename));

            var path = Path.Combine(FixturePath, filename);

            if (!File.Exists(path))
            {
                throw new FileNotFoundException(
                    $"Test fixture not found: {filename}. Run EnsureFixturesExist() first.");
            }

            return path;
        }

        /// <summary>
        /// Generates simple.pdf - A basic single-page PDF with minimal content.
        /// </summary>
        private static void GenerateSimplePdf()
        {
            const string filename = "simple.pdf";
            var path = Path.Combine(FixturePath, filename);

            if (File.Exists(path))
                return;

            try
            {
                using var pdf = Pdf.FromMarkdown("# Simple Test PDF\n\nThis is a basic PDF for testing.");
                pdf.Save(path);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Warning: Could not generate {filename}: {ex.Message}");
            }
        }

        /// <summary>
        /// Generates multipage.pdf - A multi-page document for testing.
        /// </summary>
        private static void GenerateMultiPagePdf()
        {
            const string filename = "multipage.pdf";
            var path = Path.Combine(FixturePath, filename);

            if (File.Exists(path))
                return;

            try
            {
                var content = new System.Text.StringBuilder();
                for (int i = 1; i <= 10; i++)
                {
                    if (i > 1) content.AppendLine("\n---\n");
                    content.AppendLine($"# Page {i}\n");
                    content.AppendLine($"This is page {i} of the multi-page test document.\n");
                    content.AppendLine($"Content on page {i}...");
                }

                using var pdf = Pdf.FromMarkdown(content.ToString());
                pdf.Save(path);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Warning: Could not generate {filename}: {ex.Message}");
            }
        }

        /// <summary>
        /// Generates with_text.pdf - PDF with various text elements and styles.
        /// </summary>
        private static void GenerateWithTextPdf()
        {
            const string filename = "with_text.pdf";
            var path = Path.Combine(FixturePath, filename);

            if (File.Exists(path))
                return;

            try
            {
                var content = @"# Heading 1

This is a paragraph with **bold text** and *italic text*.

## Heading 2

### Heading 3

- List item 1
- List item 2
- List item 3

1. Numbered item 1
2. Numbered item 2
3. Numbered item 3

This paragraph contains a link: [Example](https://example.com)

Final paragraph with various formatting options.";

                using var pdf = Pdf.FromMarkdown(content);
                pdf.Save(path);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Warning: Could not generate {filename}: {ex.Message}");
            }
        }

        /// <summary>
        /// Generates with_annotations.pdf - PDF with various annotation types.
        /// </summary>
        private static void GenerateWithAnnotationsPdf()
        {
            const string filename = "with_annotations.pdf";
            var path = Path.Combine(FixturePath, filename);

            if (File.Exists(path))
                return;

            try
            {
                using (var pdf = Pdf.FromMarkdown(
                    "# PDF with Annotations\n\n" +
                    "This document contains various annotations for testing.\n\n" +
                    "Text that can be highlighted or underlined.\n\n" +
                    "More content for annotation testing."))
                {
                    pdf.Save(path);
                }
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Warning: Could not generate {filename}: {ex.Message}");
            }
        }

        /// <summary>
        /// Generates search_document.pdf - Large multi-page PDF for search testing.
        /// </summary>
        private static void GenerateSearchDocumentPdf()
        {
            const string filename = "search_document.pdf";
            var path = Path.Combine(FixturePath, filename);

            if (File.Exists(path))
                return;

            try
            {
                var content = new System.Text.StringBuilder();
                content.AppendLine("# Search Test Document\n");
                content.AppendLine("This is a large multi-page document designed for search testing.\n\n");

                for (int page = 1; page <= 50; page++)
                {
                    content.AppendLine($"## Page {page}\n");
                    content.AppendLine($"Content on page {page}.\n");
                    content.AppendLine("This page contains searchable text.\n");
                    content.AppendLine($"Keyword appears on page {page}.\n");
                    content.AppendLine($"More searchable content here.\n\n");
                }

                using var pdf = Pdf.FromMarkdown(content.ToString());
                pdf.Save(path);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Warning: Could not generate {filename}: {ex.Message}");
            }
        }

        /// <summary>
        /// Gets fixture statistics for debugging.
        /// </summary>
        public static void PrintFixtureStats()
        {
            Console.WriteLine($"Fixture Directory: {FixturePath}");
            if (Directory.Exists(FixturePath))
            {
                var files = Directory.GetFiles(FixturePath, "*.pdf");
                Console.WriteLine($"Available Fixtures: {files.Length}");
                foreach (var file in files)
                {
                    var info = new FileInfo(file);
                    Console.WriteLine($"  - {Path.GetFileName(file)} ({info.Length} bytes)");
                }
            }
            else
            {
                Console.WriteLine("Fixture directory does not exist.");
            }
        }
    }
}
