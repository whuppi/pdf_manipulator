using System;
using System.Collections.Generic;
using System.Linq;
using PdfOxide.Core;
using Xunit;

namespace PdfOxide.Tests
{
    /// <summary>
    /// Tests for the v0.3.39 <c>DocumentBuilder</c> table surface
    /// (issue #393) and the accompanying primitives
    /// (<see cref="PageBuilder.Measure"/>, <see cref="PageBuilder.TextInRect"/>,
    /// <see cref="PageBuilder.StrokeRect"/>, <see cref="PageBuilder.StrokeLine"/>,
    /// <see cref="PageBuilder.NewPageSameSize"/>).
    /// </summary>
    /// <remarks>
    /// The streaming path in v0.3.39 buffers managed-side and flushes
    /// to the buffered FFI at <see cref="StreamingTable.Build"/> —
    /// these tests exercise the managed API shape; the FFI-streaming
    /// path is covered by Rust integration tests.
    /// </remarks>
    public class TablesTests
    {
        private static byte[] BuildMinimalPdf(Action<PageBuilder> body)
        {
            using var builder = DocumentBuilder.Create();
            var page = builder.LetterPage();
            body(page);
            page.Done();
            return builder.Build();
        }

        [Fact]
        public void Measure_Returns_Nonzero_For_Nonempty_Text()
        {
            using var builder = DocumentBuilder.Create();
            var page = builder.LetterPage().Font("Helvetica", 12f);
            float w = page.Measure("Hello");
            Assert.True(w > 0f, $"expected positive width, got {w}");
            // 5 chars at 12pt with 0.5 factor => 30pt
            Assert.InRange(w, 20f, 50f);
            page.Done();
        }

        [Fact]
        public void Measure_Scales_With_Font_Size()
        {
            using var builder = DocumentBuilder.Create();
            var page = builder.LetterPage();
            page.Font("Helvetica", 10f);
            float small = page.Measure("abcdefghij");
            page.Font("Helvetica", 20f);
            float big = page.Measure("abcdefghij");
            Assert.True(big > small * 1.5f,
                $"20pt measurement ({big}) should be ~2x 10pt ({small})");
            page.Done();
        }

        [Fact]
        public void TextInRect_And_StrokeRect_Render_Without_Error()
        {
            var bytes = BuildMinimalPdf(page =>
            {
                page.Font("Helvetica", 10f)
                    .TextInRect(72f, 600f, 200f, 100f,
                        "This text is wrapped inside the rectangle.",
                        Alignment.Center)
                    .StrokeRect(50f, 50f, 200f, 100f,
                        width: 2f, r: 0.5f, g: 0.5f, b: 0.5f)
                    .StrokeLine(50f, 50f, 250f, 50f,
                        width: 1f, r: 0.2f, g: 0.2f, b: 0.2f);
            });

            Assert.True(bytes.Length > 256, "expected a non-trivial PDF");
            var header = System.Text.Encoding.ASCII.GetString(bytes.Take(5).ToArray());
            Assert.Equal("%PDF-", header);
        }

        [Fact]
        public void NewPageSameSize_Adds_A_Second_Page()
        {
            var bytes = BuildMinimalPdf(page =>
            {
                page.Font("Helvetica", 12f)
                    .At(72f, 720f).Text("Page one")
                    .NewPageSameSize()
                    .At(72f, 720f).Text("Page two");
            });

            var ascii = System.Text.Encoding.ASCII.GetString(bytes);
            // Two /Type /Page entries (plus possibly /Pages parent).
            int pageOccurrences = System.Text.RegularExpressions.Regex
                .Matches(ascii, @"/Type\s*/Page\b").Count;
            Assert.True(pageOccurrences >= 2,
                $"expected >=2 /Type /Page occurrences, got {pageOccurrences}");
        }

        [Fact]
        public void Table_With_Header_Renders()
        {
            var spec = new TableSpec
            {
                Columns = new List<Column>
                {
                    new Column("SKU", 100f),
                    new Column("Qty", 60f, Alignment.Right),
                },
                Rows = new List<IList<string>>
                {
                    new[] { "A-1", "12" },
                    new[] { "B-2", "3" },
                    new[] { "C-3", "47" },
                },
                HasHeader = true,
            };

            var bytes = BuildMinimalPdf(page =>
            {
                page.Font("Helvetica", 10f)
                    .At(72f, 720f)
                    .Table(spec);
            });

            Assert.True(bytes.Length > 256);
        }

        [Fact]
        public void Table_Without_Header_Renders()
        {
            var spec = new TableSpec
            {
                Columns = new List<Column>
                {
                    new Column("", 72f),
                    new Column("", 200f),
                },
                Rows = new List<IList<string>>
                {
                    new[] { "a", "first row" },
                    new[] { "b", "second row" },
                },
                HasHeader = false,
            };

            var bytes = BuildMinimalPdf(page =>
            {
                page.Font("Helvetica", 10f)
                    .At(72f, 720f)
                    .Table(spec);
            });
            Assert.True(bytes.Length > 256);
        }

        [Fact]
        public void Table_Row_Width_Mismatch_Throws()
        {
            var spec = new TableSpec
            {
                Columns = new List<Column>
                {
                    new Column("A", 50f),
                    new Column("B", 50f),
                },
                Rows = new List<IList<string>>
                {
                    new[] { "only one cell" },
                },
            };
            using var builder = DocumentBuilder.Create();
            var page = builder.LetterPage().Font("Helvetica", 10f).At(72f, 720f);
            Assert.Throws<ArgumentException>(() => page.Table(spec));
            page.Done();
        }

        [Fact]
        public void StreamingTable_100_Rows()
        {
            using var builder = DocumentBuilder.Create();
            var page = builder.LetterPage()
                .Font("Helvetica", 8f)
                .At(72f, 720f);

            var cols = new List<Column>
            {
                new Column("SKU", 72f),
                new Column("Item", 200f),
                new Column("Qty", 48f, Alignment.Right),
            };

            using (var t = page.StreamingTable(cols, repeatHeader: true))
            {
                for (int i = 0; i < 120; i++)
                {
                    t.AddRow($"SKU-{i:D4}", $"Item number {i}", i.ToString());
                }
                t.Build();
            }

            page.Done();
            var bytes = builder.Build();
            Assert.True(bytes.Length > 1024);
        }

        [Fact]
        public void StreamingTable_Row_Width_Mismatch_Throws()
        {
            using var builder = DocumentBuilder.Create();
            var page = builder.LetterPage().Font("Helvetica", 10f).At(72f, 720f);
            var cols = new List<Column>
            {
                new Column("A", 50f),
                new Column("B", 50f),
            };
            using var t = page.StreamingTable(cols);
            Assert.Throws<ArgumentException>(() => t.AddRow("only one"));
            page.Done();
        }

        [Fact]
        public void StreamingTable_Dispose_Without_Build_Is_Safe()
        {
            using var builder = DocumentBuilder.Create();
            var page = builder.LetterPage().Font("Helvetica", 10f).At(72f, 720f);
            var cols = new List<Column> { new Column("A", 100f) };
            var t = page.StreamingTable(cols);
            t.AddRow("x");
            t.Dispose();
            // Page should still be usable — no table got drawn.
            page.At(72f, 600f).Text("after dispose");
            page.Done();
            var bytes = builder.Build();
            Assert.True(bytes.Length > 128);
        }

        [Fact]
        public void Column_Requires_Positive_Width()
        {
            Assert.Throws<ArgumentOutOfRangeException>(() => new Column("h", 0f));
            Assert.Throws<ArgumentOutOfRangeException>(() => new Column("h", -1f));
        }

        [Fact]
        public void StreamingTable_BoundedBatch_AutoFlushes()
        {
            using var builder = DocumentBuilder.Create();
            var page = builder.LetterPage()
                .Font("Helvetica", 8f)
                .At(72f, 720f);

            var cols = new List<Column>
            {
                new Column("A", 100f),
                new Column("B", 100f),
            };

            // batchSize=3, push 7 rows → 2 full flushes (batch 0..2, 3..5), 1 pending
            using var t = page.StreamingTable(cols, batchSize: 3);
            for (int i = 0; i < 7; i++)
                t.AddRow($"row{i}-a", $"row{i}-b");

            Assert.Equal(2, t.BatchCount);
            Assert.Equal(1, t.PendingRowCount);

            t.Build();

            page.Done();
            var bytes = builder.Build();
            Assert.True(bytes.Length > 256);
        }

        [Fact]
        public void StreamingTable_Flush_Drains_Buffer()
        {
            using var builder = DocumentBuilder.Create();
            var page = builder.LetterPage()
                .Font("Helvetica", 8f)
                .At(72f, 720f);

            var cols = new List<Column> { new Column("X", 150f) };
            using var t = page.StreamingTable(cols, batchSize: 100);
            t.AddRow("first");
            t.AddRow("second");
            Assert.Equal(0, t.BatchCount);
            Assert.Equal(2, t.PendingRowCount);
            t.Flush();
            Assert.Equal(1, t.BatchCount);
            Assert.Equal(0, t.PendingRowCount);
            t.Build();

            page.Done();
            builder.Build();
        }
    }
}
