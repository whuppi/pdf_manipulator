# t-511 part B: the structure tree, tagged extractAll and XFA

## Result in one paragraph

Parts 1 and 3 are done and proven. Part 2 is **not met**: the `extractAll tagged` row still fails, at
**1,568,394,791 bytes** (limit 16,777,216), down from 1,837,703,148. The structure tree is no longer what holds the
memory; the page's 600,000 text spans are. Making the row pass needs a different extraction path for a tagged page
(not a span list); that is outside this plan and I did not start it (see "What holds the memory").

## memprobe rows (release, `cargo run --release --manifest-path tool/memprobe/Cargo.toml`)

Before (the plan's diff applied, engine unchanged):

```
extractAll fonts peak_over_baseline=472062461 limit=16777216 FAIL wall_ms=5149
extractAll tagged peak_over_baseline=1837703148 limit=16777216 FAIL wall_ms=37726
```

After:

```
extractAll tagged peak_over_baseline=1568394791 limit=16777216 FAIL wall_ms=40507
extractAll fonts peak_over_baseline=472062113 limit=16777216 FAIL wall_ms=5515
```

`extractAll fonts` is the other row; reported, not worked on (472,062,113 bytes, about 450 MiB). Every other row of the
run passes (63 `ok` lines, 2 `FAIL`), including `extractAll forms` (16,179,702) and `paths` (12,887,640).

## What holds the memory (tagged page, measured before changing anything)

Release test build with the allocator counter, one thread, on the memprobe's `tagged.pdf` (600,000 elements, all on page 0):

| step | peak growth |
|---|---|
| `structure_tree()` (whole tree) | 215,567,866 |
| `traverse_structure_tree_all_pages` (all-pages content cache) | 126,429,292 |
| `extract_spans(0)` alone (600,000 spans) | 981,260,191 |
| `extract_text(0)` | 1,811,470,037 (1,542,162,028 after this change) |

(`open` itself reads the 64 MiB file into the test process, 147 MB; memprobe opens from a reader, so that is not in its number.)

File and line at HEAD:

- `src/structure/parser.rs:229` `parse_structure_tree_inner`, `:358` `parse_struct_elem`, `:525` `parse_k_children`: the recursive parse builds all 600,000 `StructElem`s (215 MB). The cap `MAX_STRUCT_ELEMENTS` (`:99`) only applied to the budgeted entry point nobody calls.
- `src/document.rs:6654`, `:12744` (and `:18202`, `:22830`): `traverse_structure_tree_all_pages` builds `OrderedContent` for every page into `structure_content_cache` (126 MB here, whole document in general).
- `src/document.rs:13096-13106` `extract_spans`: 600,000 `TextSpan`s (200 bytes each plus their strings and vectors, about 700 bytes a span), and `page_spans_cache` stores a clone (`:13106`), so two copies exist at the peak.
- `src/document.rs:11613`, `:12819` (`mcid_map`: a `Vec<TextSpan>` per (scope, MCID), 600,000 of them) in the structure-order assembler.

Parts 1 and 3 removed the first two. The last two are the page's own text and remain. A tagged page of 600,000 marked runs
has to be read as spans, grouped by MCID and emitted in tree order; nothing in the plan changes that. I measured it rather
than guess: `page_structure` for this page is 146,974,131 bytes by itself (600,000 `OrderedContent`s, bounded by the page as
the plan asks), and the spans are several times that.

## What I changed (all in `vendor/pdf_oxide`)

1. **Structure tree walk** (`src/structure/parser.rs`, one patch block). `walk_structure_tree` visits elements depth-first
   with an explicit stack of child lists, resolves each element when it is reached and drops it after, and keeps a visited
   set of `(id, gen)` pairs so cycles end. It reports events to a `StructVisitor`. `parse_structure_tree` /
   `parse_structure_tree_with_budget` are now a `Builder` over that walk (same tree as before, no recursion, no cap).
   `MAX_STRUCT_ELEMENTS` and the element count in `ParseBudget` are gone; the wall-clock budget stays.
2. **Per-page products** (`src/structure/traversal.rs`, `table_extractor.rs`, one patch block each). `page_structure`
   produces, in one walk, a page's `OrderedContent` list and its `Table` elements; `streamed_actualtext_index` produces the
   ActualText index. Word-break elements, `Table` elements and elements bearing `/ActualText` are held whole while their
   subtree is read (their semantics look ahead into the subtree) and the existing functions are applied to them, so the
   results equal the whole-tree functions' (proved below).
3. **Consumers** (`src/document.rs`). The tree cache now holds only the header (`StructHeader`: role map, root `/K`, page map).
   `structure_content_cache` (all pages) and `table_elements_cache` (all pages) are replaced by `structure_page_cache`, the
   structure of the page last asked for. `struct_tree_trustworthy` / `struct_tree_marked` return the header; `actualtext_index`
   uses the streamed index.
4. **XFA as events** (`src/xfa/extractor.rs`, `parser.rs`, `integration.rs`, `mod.rs`). `XfaExtractor::read_packets` hands each
   packet to a callback as a `BufRead` over the stream decoders' reader (`decoders::stream::reader_in`, as `xmp.rs` does);
   `parse_form` feeds them to `XfaParser::feed`, which parses template and datasets sections with `quick_xml` events.
   `extract_xfa`, `extract_xfa_packets` and their two array helpers (the four `read_whole_raw` callers) are removed, as are
   `XfaForm::template_xml` / `datasets_xml`. `analyze_xfa_document` and `convert_xfa_document` keep their results.
5. `src/host/engine_proofs/whole_reads.txt`: the four `xfa::extractor::*` lines removed, as that file's rule says.

Grep for the removed names (`extract_xfa`, `template_xml`, `datasets_xml`, `structure_content_cache`, `table_elements_cache`,
`MAX_STRUCT_ELEMENTS`) across the repository finds nothing outside this change except `vendor/pdf_oxide/js/API_GUIDE.md:653`,
the JavaScript binding's own `XFAForm.new(template_xml)`, which is a different type. Nothing outside `src/xfa/` and the host's
`dispatch::doc_xfa` / `dispatch::convert_xfa_to_acroform` (through `analyze_xfa_document` / `convert_xfa_document`) read the XFA
strings; the only other users of the structure tree's whole form are `structure_tree()` (public, unchanged) and the `ocr`-feature
`to_markdown_with_ocr`, which I left on it.

## Tests (new files only, in `src/host/engine_proofs/`)

`mem_structure.rs`
- `a_tree_of_200000_elements_is_walked_for_one_page_in_bounded_memory`: 200,000 paragraphs over 200 pages, one page walked:
  **15,113,614 bytes** of growth on the thread (the document's fixed 8 MiB object cache is most of it) against **79,606,230**
  for the same tree held whole; the page gets exactly MCIDs 0..999.
- `a_tree_past_the_old_cap_is_returned_whole_even_under_a_budget`: 20,000 elements, with and without a budget. It replaces
  `parse_budget_bounded_caps_element_count` (removed from `parser.rs` under a patch marker).
- `a_cyclic_tree_ends`: the document listed twice at the root, a section listing its parent and itself, an indirect `/K` pointing
  back up; four elements, each once.
- `a_walk_gives_what_the_whole_tree_traversals_give`: per page, `page_structure` equals
  `traverse_structure_tree_all_pages` and `find_table_elements_all_pages` field by field (headings via RoleMap, lists, a table, a
  word break), and `streamed_actualtext_index` equals `build_actualtext_index` (nested scope, scope across two pages).

`mem_xfa.rs`
- `a_64_mib_datasets_packet_is_analysed_as_events`: **210,472 bytes** of growth for `analyze_xfa_document` on a 64 MiB datasets
  packet (limit 1 MiB); `parse_form` also under 1 MiB and the field gets its value.
- `one_xdp_stream_and_an_array_of_packets_give_the_same_form`.

Every `#[test]` carries the `///` doc comment the plan asks for. The two memory-heavy structure tests run in a child process
(`alone.rs`) because the existing counter is process-wide and its readers (`mem_fixture`, `page_operators`, `filter_readers`)
assert growth of 1 MiB to 32 MiB: my first run of the whole filter had `mem_fixture` fail with 80,284,800 bytes of growth, which is
my 79.6 MB whole-tree measure. I also added a per-thread counter (`thread_growth`) beside the existing one in `filter_readers.rs`
(allocator code only; no test in that file was touched) so my bounds are not hostage to other threads.

### Each shown to fail (break, run, restore, `git diff` identical)

| proof | injected break | failing message |
|---|---|---|
| 200,000 walk | `PageWalk::marked_content` without the page filter | `assertion left == right failed` on the page's MCIDs (`[0, 0, 0, ...` all pages), growth 45,489,620 |
| past the old cap | a 10,000 element stop in `walk_structure_tree` | `panicked at mem_structure.rs:235` (`an hour is not exceeded`: the budgeted parse returned `None`) |
| cyclic tree | cycle guard `first_visit` always true | process aborted, `memory allocation of 264 bytes failed`, exit 134 (4 GB limit) |
| equivalence (content) | captured subtrees start from the default context | `assertion left == right failed: page 0`, `section_id: Some(1)` expected, `None` got |
| equivalence (ActualText) | ActualText subtrees not captured | `left: {(Page(1), 6), (Page(0), 5), (Page(1), 7), (Page(1), 8)}  right: {}` |
| 64 MiB XFA | `read_packets` reads the stream whole | `analysing a 64 MiB packet grew the thread by 67245625 bytes` |
| XDP vs packets | `datasets`/`data` sections not recognised | `left: None  right: Some("Ada")` |

## Gate (summary lines)

```
engine_proofs (--test-threads=1): test result: ok. 415 passed; 0 failed; 3 ignored; 0 measured; 6204 filtered out; finished in 1561.54s
structure:                        test result: ok. 251 passed; 0 failed; 2 ignored; 0 measured; 6369 filtered out; finished in 29.63s
xfa:                              test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 6576 filtered out; finished in 54.94s
tagged:                           test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 6604 filtered out; finished in 0.02s
extract:                          test result: ok. 812 passed; 0 failed; 0 ignored; 0 measured; 5810 filtered out; finished in 1.11s
```

The 3 and 2 ignored are my `alone` bodies, run by their parent tests.

**`engine_proofs` run in parallel (the default) is not clean, and it is not because of this change.** Two existing tests,
`filter_readers::a_64_mib_stream_through_ascii85_and_flate_is_never_held` and
`page_operators::a_page_of_96_mib_of_flate_content_iterates_in_under_a_mebibyte_of_growth`, assert under 1 MiB of process-wide
growth and fail when other threads allocate: they failed the same way with `mem_structure` and `mem_xfa` skipped
(`--skip mem_structure --skip mem_xfa`: 11,223,717 and 128,148,798 bytes of growth) and pass alone and with `--test-threads=1`. I did not edit them.

## Choices (where the plan did not settle one, the narrower)

- **Page walks are per page.** Each page's structure costs one walk of the whole tree, cached for the last page only. That is the
  plan's "bounded by the page, not the document", and it makes extracting every page of a large tagged document quadratic in
  tree reads where the old all-pages cache was linear. Worth a decision by the maintainer.
- **A tree that cannot be walked** (an object that fails to load mid-walk) gives that page an empty structure with a warning. Before,
  the whole tree failed to parse and the document fell back to geometric order. The gate for "tagged" is now the header only.
- **Budget exceeded** anywhere in the walk returns `None` (as the documented behaviour says); the old code returned a partial tree
  when the deadline passed below the root.
- **An ActualText-bearing subtree is held whole** while its scope is read, as are word-break and table subtrees. An `/ActualText` on
  the document root would hold the document.
- **Existing test edited for compilation:** `parse_budget_unbounded_never_exceeds` named the removed constant; its literal `10_000` replaces
  it (patch marker in place). No assertion changed. `parse_budget_bounded_caps_element_count` is removed under the plan's ruling.
- **XFA test-only helpers:** `get_filters` and `decode_stream_data` are `#[cfg(test)]` now (their existing tests pin them); nothing else calls them.
- **XFA parse differences:** a section without its closing tag is parsed to the end (the string version ignored it), and an XML error
  outside a template/datasets section ends the packet quietly (the string version never read outside them).
- **Patch markers:** `src/document.rs` has three more `end` markers than openers at HEAD (first at line 24043 there); I left them. My
  blocks are balanced and not nested in every file I touched.
- **Not done:** I did not run `make check` (needs Dart and FVM) or the wasm build.
- **Commit:** no AI co-author trailer, per `AGENTS.md`.
