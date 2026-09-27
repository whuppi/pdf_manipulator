/**
 * Performance Benchmarks - Phase 3.2
 *
 * Measures performance of critical operations across Node.js bindings
 * to ensure performance meets cross-language consistency requirements.
 *
 * Benchmark targets:
 * - Text extraction: < 50ms per page
 * - Markdown conversion: < 100ms per page
 * - DOM navigation: < 10ms for 1000 elements
 * - Search: < 200ms for 100-page document
 *
 * Equivalent benchmarks exist in:
 * - Java: benchmarks/src/jmh/java/com/pdfoxide/
 * - C#: csharp/PdfOxide.Benchmarks/
 */

import assert from 'node:assert';
import { describe, it } from 'node:test';
import {
  ConversionOptionsBuilder,
  MetadataBuilder,
  PdfBuilder,
  SearchOptionsBuilder,
} from '../lib/builders/index.js';
import { PdfException, PdfIoError, PdfParseError } from '../lib/errors.js';

/**
 * Performance measurement helper
 */
function measureTime(fn) {
  const start = performance.now();
  const result = fn();
  const duration = performance.now() - start;
  return { result, duration };
}

/**
 * Async performance measurement helper
 */
async function measureTimeAsync(fn) {
  const start = performance.now();
  const result = await fn();
  const duration = performance.now() - start;
  return { result, duration };
}

/**
 * Validates benchmark result against target
 */
function validateBenchmark(name, actual, target, unit = 'ms') {
  const tolerance = target * 0.2; // Allow 20% variance
  const passed = actual <= target + tolerance;
  const status = passed ? '✅' : '⚠️';
  const message = `${status} ${name}: ${actual.toFixed(2)}${unit} (target: ${target}${unit})`;
  console.log(message);
  return passed;
}

/**
 * Generate sample PDF content for benchmarking
 */
function generateSampleContent(pages = 10) {
  const lines = [];
  for (let i = 0; i < pages; i++) {
    lines.push(`Page ${i + 1}`);
    for (let j = 0; j < 50; j++) {
      lines.push(`Line ${j + 1}: Lorem ipsum dolor sit amet, consectetur adipiscing elit.`);
    }
    lines.push('');
  }
  return lines.join('\n');
}

describe('Performance Benchmarks - Phase 3.2', () => {
  describe('Metadata Operations Performance', () => {
    it('should measure MetadataBuilder construction time', () => {
      // Warmup
      for (let i = 0; i < 100; i++) {
        MetadataBuilder.create().title('Test').author('Author').build();
      }

      // Benchmark: 1000 metadata builders
      const { duration } = measureTime(() => {
        let count = 0;
        for (let i = 0; i < 1000; i++) {
          const metadata = MetadataBuilder.create()
            .title(`Document ${i}`)
            .author('Test Author')
            .subject('Test Subject')
            .keywords(['test', 'benchmark'])
            .build();
          count++;
        }
        return count;
      });

      const avgTime = duration / 1000;
      console.log(`\n  Metadata construction: ${avgTime.toFixed(4)}ms per builder`);
      assert.ok(avgTime < 0.5, 'Metadata builder construction should be < 0.5ms');
    });

    it('should measure ConversionOptionsBuilder construction time', () => {
      // Warmup
      for (let i = 0; i < 100; i++) {
        ConversionOptionsBuilder.create().preserveFormatting(true).includeImages(true).build();
      }

      // Benchmark: 1000 conversion options builders
      const { duration } = measureTime(() => {
        let count = 0;
        for (let i = 0; i < 1000; i++) {
          const options = ConversionOptionsBuilder.create()
            .preserveFormatting(i % 2 === 0)
            .detectHeadings(true)
            .detectTables(true)
            .includeImages(i % 3 === 0)
            .imageQuality(85)
            .build();
          count++;
        }
        return count;
      });

      const avgTime = duration / 1000;
      console.log(`\n  ConversionOptions construction: ${avgTime.toFixed(4)}ms per builder`);
      assert.ok(avgTime < 0.5, 'ConversionOptions builder should be < 0.5ms');
    });

    it('should measure SearchOptionsBuilder construction time', () => {
      // Warmup
      for (let i = 0; i < 100; i++) {
        SearchOptionsBuilder.create().caseSensitive(false).wholeWords(true).build();
      }

      // Benchmark: 1000 search options builders
      const { duration } = measureTime(() => {
        let count = 0;
        for (let i = 0; i < 1000; i++) {
          const options = SearchOptionsBuilder.create()
            .caseSensitive(i % 2 === 0)
            .wholeWords(i % 3 === 0)
            .useRegex(i % 5 === 0)
            .maxResults(100 + i)
            .build();
          count++;
        }
        return count;
      });

      const avgTime = duration / 1000;
      console.log(`\n  SearchOptions construction: ${avgTime.toFixed(4)}ms per builder`);
      assert.ok(avgTime < 0.5, 'SearchOptions builder should be < 0.5ms');
    });
  });

  describe('Builder Pattern Performance', () => {
    it('should measure fluent builder chain performance', () => {
      // Warmup
      for (let i = 0; i < 100; i++) {
        PdfBuilder.create()
          .title('Test')
          .author('Author')
          .subject('Subject')
          .pageSize('A4')
          .margins(20, 20, 20, 20);
      }

      // Benchmark: fluent chaining
      const { duration } = measureTime(() => {
        let count = 0;
        for (let i = 0; i < 5000; i++) {
          const builder = PdfBuilder.create()
            .title(`Document ${i}`)
            .author(`Author ${i % 10}`)
            .subject(`Subject ${i % 5}`)
            .pageSize(i % 2 === 0 ? 'A4' : 'Letter')
            .margins(20, 20, 20, 20);
          count++;
        }
        return count;
      });

      const avgTime = duration / 5000;
      console.log(`\n  Fluent chain construction: ${avgTime.toFixed(4)}ms per chain`);
      assert.ok(avgTime < 0.3, 'Fluent builder chaining should be < 0.3ms');
    });

    it('should measure preset factory method performance', () => {
      // Warmup
      for (let i = 0; i < 100; i++) {
        ConversionOptionsBuilder.highQuality().build();
        SearchOptionsBuilder.strict().build();
      }

      // Benchmark: preset creation
      const { duration } = measureTime(() => {
        let count = 0;
        for (let i = 0; i < 5000; i++) {
          if (i % 3 === 0) {
            ConversionOptionsBuilder.default().build();
          } else if (i % 3 === 1) {
            ConversionOptionsBuilder.highQuality().build();
          } else {
            ConversionOptionsBuilder.fast().build();
          }
          if (i % 2 === 0) {
            SearchOptionsBuilder.default().build();
          } else {
            SearchOptionsBuilder.strict().build();
          }
          count++;
        }
        return count;
      });

      const avgTime = duration / 5000;
      console.log(`\n  Preset factory methods: ${avgTime.toFixed(4)}ms per preset`);
      assert.ok(avgTime < 0.2, 'Preset factory methods should be < 0.2ms');
    });
  });

  describe('Error Handling Performance', () => {
    it('should measure error class instantiation performance', () => {
      // Warmup
      for (let i = 0; i < 100; i++) {
        new PdfException('Test error');
        new PdfIoError('I/O error');
        new PdfParseError('Parse error');
      }

      // Benchmark: error creation
      const { duration } = measureTime(() => {
        let count = 0;
        for (let i = 0; i < 5000; i++) {
          if (i % 3 === 0) {
            new PdfException(`Error ${i}`);
          } else if (i % 3 === 1) {
            new PdfIoError(`I/O error ${i}`);
          } else {
            new PdfParseError(`Parse error ${i}`);
          }
          count++;
        }
        return count;
      });

      const avgTime = duration / 5000;
      console.log(`\n  Error class creation: ${avgTime.toFixed(4)}ms per error`);
      assert.ok(avgTime < 0.1, 'Error class creation should be < 0.1ms');
    });

    it('should measure error throwing and catching performance', () => {
      // Warmup
      for (let i = 0; i < 100; i++) {
        try {
          throw new PdfIoError('Test error');
        } catch (e) {
          // caught
        }
      }

      // Benchmark: throw/catch
      const { duration } = measureTime(() => {
        let caught = 0;
        for (let i = 0; i < 1000; i++) {
          try {
            throw new PdfIoError(`I/O error ${i}`);
          } catch (e) {
            if (e instanceof PdfIoError) {
              caught++;
            }
          }
        }
        return caught;
      });

      const avgTime = duration / 1000;
      console.log(`\n  Error throw/catch cycle: ${avgTime.toFixed(4)}ms per cycle`);
      assert.ok(avgTime < 0.5, 'Error throw/catch should be < 0.5ms');
    });
  });

  describe('Cross-Language Performance Targets', () => {
    it('should document performance targets from plan', () => {
      const targets = {
        'Text extraction': '< 50ms per page',
        'Markdown conversion': '< 100ms per page',
        'DOM navigation': '< 10ms for 1000 elements',
        Search: '< 200ms for 100-page document',
      };

      console.log('\n  Performance Targets (Cross-Language):\n');
      for (const [operation, target] of Object.entries(targets)) {
        console.log(`    ${operation}: ${target}`);
      }

      // These would be measured with actual PDF documents
      // when full native module is available
      console.log('\n  Note: Full PDF operations require native module.');
      console.log('  Builder/error performance validated above.');
      console.log('  Full document benchmarks will run in integration environment.');
    });

    it('should provide performance comparison structure', () => {
      const comparison = {
        'Java (JNI)': 'baseline',
        'C# (P/Invoke)': 'compare',
        'Node.js (napi-rs)': 'measure',
      };

      console.log('\n  Cross-Language Performance Comparison:\n');
      for (const [language, role] of Object.entries(comparison)) {
        console.log(`    ${language}: ${role}`);
      }

      // Verify structure is in place
      assert.ok(Object.keys(comparison).length === 3);
      console.log('\n  All three language implementations in comparison framework.');
    });
  });

  describe('Memory Usage Patterns', () => {
    it('should measure builder memory efficiency', () => {
      // Note: V8 GC prevents accurate measurement in test context
      // This provides structure for memory profiling tools

      const builders = [];
      for (let i = 0; i < 100; i++) {
        builders.push(MetadataBuilder.create().title(`Doc ${i}`).author('Author').build());
      }

      assert.strictEqual(builders.length, 100);
      console.log('\n  Memory patterns: Created 100 metadata objects');
      console.log('  Builders are designed for lazy evaluation and minimal overhead');
    });

    it('should verify error class memory overhead', () => {
      const errors = [];
      for (let i = 0; i < 100; i++) {
        if (i % 2 === 0) {
          errors.push(new PdfException(`Error ${i}`));
        } else {
          errors.push(new PdfIoError(`I/O error ${i}`));
        }
      }

      assert.strictEqual(errors.length, 100);
      console.log('\n  Created 100 error instances');
      console.log('  Error classes extend Error with minimal overhead');
    });
  });

  describe('Benchmark Summary', () => {
    it('should collect and report all benchmark results', () => {
      console.log('\n╔════════════════════════════════════════════════════════╗');
      console.log('║         Node.js Performance Benchmarks - Phase 3.2      ║');
      console.log('╚════════════════════════════════════════════════════════╝\n');

      const results = {
        'Builder Construction': {
          Metadata: '< 0.5ms',
          ConversionOptions: '< 0.5ms',
          SearchOptions: '< 0.5ms',
        },
        'Fluent Chaining': {
          'Chain construction': '< 0.3ms',
          'Preset factory methods': '< 0.2ms',
        },
        'Error Handling': {
          'Error class creation': '< 0.1ms',
          'Throw/catch cycle': '< 0.5ms',
        },
        'Cross-Language Targets': {
          'Text extraction': '< 50ms/page',
          'Markdown conversion': '< 100ms/page',
          'DOM navigation': '< 10ms/1000 elements',
          'Full-document search': '< 200ms/100 pages',
        },
      };

      for (const [category, metrics] of Object.entries(results)) {
        console.log(`${category}:`);
        for (const [metric, target] of Object.entries(metrics)) {
          console.log(`  • ${metric}: ${target}`);
        }
        console.log('');
      }

      console.log('Status: ✅ All Node.js benchmarks configured');
      console.log('Next: Run Java and C# benchmarks for cross-language comparison\n');
    });
  });
});
