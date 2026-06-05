# PDF Corpus Verification Results — v0.3.5

**Date**: 2026-02-15
**Tool**: `examples/verify_corpus.rs` (thread-based 120s timeout)
**Corpora**: veraPDF (2,907) + pdf_oxide_tests (1,313) + pdfium (836) = **5,056 PDFs**

## Summary

| Metric | Count | % |
|--------|------:|----:|
| **Pass** | 4,992 | 98.7% |
| **Slow (>5s)** | 53 | 1.0% |
| **Fail** | 8 | 0.2% |
| **Timeout (>120s)** | 3 | 0.1% |
| **Panic** | 0 | 0.0% |
| **Success rate** | **5,045** | **99.8%** |

## Timing Distribution (5,045 successful PDFs)

```
     <10ms:  4,396 (87.1%) ##################################
   10-50ms:    296 ( 5.9%) ##
  50-100ms:     70 ( 1.4%) #
 100-500ms:    188 ( 3.7%) #
    0.5-1s:     16 ( 0.3%) #
      1-5s:     26 ( 0.5%) #
     5-10s:      9 ( 0.2%) #
    10-30s:     24 ( 0.5%) #
    30-60s:     12 ( 0.2%) #
      >60s:      8 ( 0.2%) #
```

**Percentiles**: min=0.1ms, p50=1.0ms, p90=19.8ms, p95=120.0ms, p99=5,493ms, max=119,753ms

## Per-Corpus Breakdown

| Corpus | Total | Pass | Fail | Slow | T/O | Success |
|--------|------:|-----:|-----:|-----:|----:|--------:|
| veraPDF | 2,907 | 2,906 | 0 | 1 | 0 | 100.0% |
| pdfium | 836 | 819 | 2 | 15 | 0 | 99.8% |
| pdfjs | 897 | 888 | 6 | 1 | 2 | 99.1% |
| pdf_oxide_tests | 367 | 335 | 0 | 31 | 1 | 99.7% |
| safedocs | 26 | 26 | 0 | 0 | 0 | 100.0% |

## Failures (8)

All failures are on deliberately broken, fuzzed, or adversarial test files except `2_halftone.pdf`.

| File | Corpus | Error | Type |
|------|--------|-------|------|
| bug1606566.pdf | pdfjs | No PDF header in first 8192 bytes | Deliberately missing version header |
| bug_511_jbig_null_crash.pdf | pdfium | No PDF header in first 8192 bytes | JBIG2 crash test file |
| bug1020226.pdf | pdfjs | Expected Dictionary, found Null | Truncated inline image test |
| poppler-742-0-fuzzed.pdf | pdfjs | Expected Dictionary, found Null | Fuzzed/mutated PDF |
| 2_halftone.pdf | pdfium | Invalid cross-reference table | **Real bug** (Issue #71) |
| issue9105_other.pdf | pdfjs | /Root is not a reference | Deliberately malformed |
| Pages-tree-refs.pdf | pdfjs | Page index 1 not found by scanning | Circular page tree test |
| REDHAT-1531897-0.pdf | pdfjs | Failed to parse xref | Integer overflow in /W array |

## Timeouts (3)

| File | Corpus | Size | Issue |
|------|--------|-----:|-------|
| PDF32000_2008.pdf | pdf_oxide_tests | 22 MB | Real PDF, performance bug (Issue #73) |
| poppler-67295-0.pdf | pdfjs | 794 B | DoS: `/Count 9999999999` (Issue #72) |
| poppler-85140-0.pdf | pdfjs | 340 B | DoS: `/Count 213804087` + integer overflow (Issue #72) |

## Slow PDFs (53) — Performance Concerns

### Category 1: 10,000-Page Conformance Test (Issue #76)

| File | Pages | MB | Time | pg/s |
|------|------:|---:|-----:|-----:|
| isartor-6-1-12-t01-fail-a.pdf | 10,000 | 3.8 | 120s | 83.5 |

**Root cause**: Per-page overhead (12ms/page). Target: <1ms/page for simple pages.

### Category 2: Giant Newspaper Archives (Issue #77)

| File | Pages | MB | Time | pg/s |
|------|------:|----:|-----:|-----:|
| IA_001-jan.-4-1940-dec.-30-1941a.pdf | 436 | 1,613 | 110s | 4.0 |
| IA_001-jan.-4-1940-dec.-30-1941b.pdf | 427 | 1,551 | 109s | 3.9 |
| IA_002-jan.-8-1942-dec.-29-1943a.pdf | 422 | 1,440 | 103s | 4.1 |
| IA_002-jan.-8-1942-dec.-29-1943b.pdf | 428 | 1,406 | 97s | 4.4 |
| IA_01021812.pdf | 24 | 12 | 10s | 2.5 |

**Root cause**: I/O dominated for the 1.5 GB files (~14 MB/s throughput). The small IA_01021812.pdf at 2.5 pg/s (500ms/page for 12 MB) is the worst outlier — likely scanned images with heavy JPEG/JBIG2 streams.

### Category 3: CFR Government Documents (Issues #75, #78)

24 CFR (Code of Federal Regulations) documents from US Government Publishing Office. All text-heavy legal documents, 478–1,322 pages each.

| File | Pages | MB | Time | pg/s |
|------|------:|---:|-----:|-----:|
| CFR Title16 (Commercial Practices) | 794 | 40.2 | 96s | **8.3** |
| CFR Title47 (Telecommunication) | 1,191 | 18.2 | 86s | 13.8 |
| CFR Title17 (Securities) | 1,010 | 17.6 | 64s | 15.8 |
| CFR Title18 (Power/Water) | 1,322 | 8.3 | 56s | 23.5 |
| CFR Title14 (Aeronautics) | 942 | 12.5 | 44s | 21.2 |
| CFR Title10 (Energy) | 1,121 | 6.2 | 42s | 26.4 |
| CFR Title12 (Banking) | 1,191 | 5.5 | 41s | 29.1 |
| CFR Title08 (Immigration) | 1,281 | 3.8 | 41s | 31.6 |
| CFR Title24 (Housing) | 832 | 3.7 | 38s | 22.0 |
| CFR Title45 (Public Welfare) | 876 | 4.1 | 37s | 23.8 |
| CFR Title37 (Patents) | 1,106 | 3.8 | 35s | 31.7 |
| CFR Title27 (Alcohol/Firearms) | 1,091 | 3.7 | 34s | 32.1 |
| CFR Title43 (Public Lands) | 790 | 4.0 | 33s | 24.0 |
| CFR Title38 (Veterans) | 1,009 | 4.4 | 32s | 31.7 |
| CFR Title42 (Public Health) | 1,042 | 3.7 | 31s | 33.8 |
| CFR Title33 (Navigation) | 758 | 3.0 | 29s | 25.9 |
| CFR Title49 (Transportation) | 782 | 7.8 | 26s | 30.3 |
| CFR Title19 (Customs) | 1,130 | 4.2 | 25s | 45.8 |
| CFR Title30 (Minerals) | 836 | 5.0 | 25s | 33.9 |
| CFR Title44 (Emergency Mgmt) | 606 | 2.5 | 22s | 27.7 |
| CFR Title29 (Labor) | 818 | 3.8 | 22s | 37.7 |
| CFR Title40 (Environment) | 849 | 3.4 | 22s | 39.3 |
| CFR Title20 (Benefits) | 687 | 2.8 | 15s | 44.4 |
| CFR Title21 (Food/Drugs) | 643 | 3.1 | 15s | 44.3 |
| cfr_excerpt.pdf | 660 | 2.5 | 13s | 50.2 |
| CFR Title07 (Agriculture) | 660 | 2.5 | 13s | 51.6 |
| CFR Title15 (Commerce) | 478 | 2.3 | 9s | 53.1 |
| CFR Title34 (Education) | 619 | 2.5 | 12s | 50.1 |

**Average**: 32.5 pg/s across all CFR docs. **Target**: >100 pg/s for text-heavy PDFs.

### Category 4: FRC 8.2.4 Embedded File PDFs (Issue #79)

14 FRC (File Reference Conformance) test PDFs from pdfium. All ~3.3 MB with **0 extractable pages** but taking 5–13 seconds just to open.

| File | Pages | MB | Open time |
|------|------:|---:|---------:|
| FRC_13_8.2.4_remove_Size_value.pdf | 0 | 3.3 | 12.9s |
| FRC_24_8.2.4__remove_Order_obj.pdf | 0 | 3.3 | 12.3s |
| FRC_20_8.2.4__remove_CreationDate_all.pdf | 0 | 3.3 | 11.7s |
| FRC_17_8.2.4__remove_CompressedSize__all.pdf | 0 | 3.3 | 11.6s |
| FRC_5_8.2.4__remove_FileName_all.pdf | 0 | 3.3 | 11.5s |
| FRC_7_8.2.4__remove_Description_value.pdf | 0 | 3.3 | 11.3s |
| FRC_1_8.2.4__original.pdf | 0 | 3.3 | 11.2s |
| FRC_23_8.2.4__remove_Order_all.pdf | 0 | 3.3 | 11.1s |
| FRC_11_8.2.4__remove_ModDate_all.pdf | 0 | 3.3 | 10.9s |
| FRC_22_8.2.4__remove_Order_value.pdf | 0 | 3.3 | 10.0s |
| FRC_8_8.2.4__remove__Description_all.pdf | 0 | 3.3 | 9.4s |
| FRC_19_8.2.4__remove_CreationDate_value.pdf | 0 | 3.3 | 7.4s |
| FRC_4_8.2.4__remove_FileName_value.pdf | 0 | 3.3 | 6.0s |
| FRC_9_8.2.4__remove__Description_obj.pdf | 0 | 3.3 | 5.3s |
| FRC_6_8.2.4__remove_FileName_obj.pdf | 0 | 3.3 | 5.2s |

**Root cause**: These are PDF portfolio/collection files with embedded file attachments but no displayable pages. The parser appears to spend excessive time on xref reconstruction or stream decoding during open, even though there are no pages to extract.

### Category 5: Individual Outliers (Issue #74)

| File | Pages | MB | Time | pg/s |
|------|------:|---:|-----:|-----:|
| RLGNJP7L3BZWPR6KCTTN5I4DIPFSCP3L.pdf | 73 | 2.0 | 23s | **3.2** |
| 2Z5VOQ6G6CMR5GMVSAAXULXHXTMJPTM2.pdf | 174 | 0.5 | 5.5s | 31.5 |
| freeculture.pdf | 352 | 2.5 | 5.5s | 64.1 |

**RLGNJP7L3BZWPR6KCTTN5I4DIPFSCP3L.pdf** is the worst performance outlier: 15-30x slower than comparable files.

## GitHub Issues Created

| Issue | Type | Category | Files |
|------:|------|----------|------:|
| #71 | bug | Invalid xref on valid PDF | 1 |
| #72 | bug | DoS via bogus /Count | 2 |
| #73 | perf | PDF spec timeout | 1 |
| #74 | perf | Individual outlier (3.2 pg/s) | 1 |
| #75 | perf | CFR Title 16 (8.3 pg/s) | 1 |
| #76 | perf | 10K-page overhead (12ms/page) | 1 |
| #77 | perf | Giant newspaper archives | 5 |
| #78 | perf | CFR batch (24 docs, avg 32 pg/s) | 24 |
| #79 | perf | FRC 8.2.4 slow open (0 pages) | 14 |

## Performance Targets

| Scenario | Current | Target | Gap |
|----------|--------:|-------:|----:|
| Simple pages (isartor) | 83 pg/s | 1,000 pg/s | 12x |
| Text-heavy (CFR avg) | 33 pg/s | 100 pg/s | 3x |
| Complex pages (outlier) | 3.2 pg/s | 50 pg/s | 16x |
| 0-page portfolio open | 5-13s | <1s | 5-13x |
| PDF spec (22MB, 756pg) | >120s | <30s | >4x |
