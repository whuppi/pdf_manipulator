// Cross-language benchmark — C# binding.
//
// Emits NDJSON matching the Rust baseline (bench/rust_bench.rs) so results
// can be aggregated and compared.
//
// Run with:
//   dotnet run --project csharp/PdfOxide.Bench -c Release -- bench_fixtures/tiny.pdf ...
using System;
using System.Diagnostics;
using System.IO;
using System.Text.Json;

namespace PdfOxide.Bench;

internal static class Program
{
    private const int Iterations = 5;

    private record FixtureResult(
        string Language,
        string Fixture,
        long SizeBytes,
        long OpenNs,
        long ExtractPage0Ns,
        long ExtractAllNs,
        long SearchNs,
        int PageCount,
        int TextLen);

    private static long NanosSince(long startTicks)
    {
        long elapsedTicks = Stopwatch.GetTimestamp() - startTicks;
        return (long)(elapsedTicks * (1_000_000_000.0 / Stopwatch.Frequency));
    }

    private static FixtureResult BenchFixture(string path)
    {
        var size = new FileInfo(path).Length;
        var fixture = Path.GetFileName(path);

        // Warm-up pass (not measured) — exercises every code path we're
        // about to measure so per-call JIT costs are amortized away.
        {
            using var doc = PdfOxide.Core.PdfDocument.Open(path);
            _ = doc.ExtractText(0);
            _ = doc.SearchAll("the", false);
        }

        // Open (average).
        long openTotal = 0;
        for (int i = 0; i < Iterations; i++)
        {
            long start = Stopwatch.GetTimestamp();
            using var doc = PdfOxide.Core.PdfDocument.Open(path);
            openTotal += NanosSince(start);
        }
        long openNs = openTotal / Iterations;

        // Extract page 0 (average) + all pages + search on a single open doc.
        using var reused = PdfOxide.Core.PdfDocument.Open(path);
        int pageCount = reused.PageCount;

        long p0Total = 0;
        int textLen = 0;
        for (int i = 0; i < Iterations; i++)
        {
            long start = Stopwatch.GetTimestamp();
            var text = reused.ExtractText(0);
            p0Total += NanosSince(start);
            // Report UTF-8 byte count (not string.Length which counts UTF-16
            // code units) so the number is directly comparable to Go/Rust
            // across the bench harness.
            textLen = text == null ? 0 : System.Text.Encoding.UTF8.GetByteCount(text);
        }
        long extractPage0Ns = p0Total / Iterations;

        long allStart = Stopwatch.GetTimestamp();
        for (int i = 0; i < pageCount; i++)
        {
            _ = reused.ExtractText(i);
        }
        long extractAllNs = NanosSince(allStart);

        long searchStart = Stopwatch.GetTimestamp();
        _ = reused.SearchAll("the", false);
        long searchNs = NanosSince(searchStart);

        return new FixtureResult(
            "csharp",
            fixture,
            size,
            openNs,
            extractPage0Ns,
            extractAllNs,
            searchNs,
            pageCount,
            textLen);
    }

    public static int Main(string[] args)
    {
        if (args.Length == 0)
        {
            Console.Error.WriteLine("usage: csharp_bench <fixture.pdf>...");
            return 1;
        }

        var options = new JsonSerializerOptions
        {
            PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        };

        foreach (var path in args)
        {
            try
            {
                var result = BenchFixture(path);
                Console.WriteLine(JsonSerializer.Serialize(result, options));
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"csharp_bench failed for {path}: {ex.Message}");
                return 2;
            }
        }
        return 0;
    }
}
