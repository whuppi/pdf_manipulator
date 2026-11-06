# t-574 report: whole buffers in images, fonts, colour, compliance, decoders, TSA and PNG embedding

Lint, before and after (`bash tool/check_whole_buffers.sh`):

    before: 59 site(s) read a whole buffer that is not a table
    after:  50 site(s) read a whole buffer that is not a table

Nine sites in my share left the list: `add_pdfa_identification#1,#2`, `resolve_indexed_palette#1`,
`decode_jbig2_image#1,#2`, `parse_descendant_fonts#1`, `image_embed::take#1`, `color.rs::resolve_indexed#1`,
`tsa_client::post#1`. Three of my sites stay, and I say why below: `extract_image_from_xobject_at#1` (the
JPEG fallback), `#2` (a fax stream) and `decode_slice#1` (item 6).

## Status by item

| # | Item | Status |
|---|------|--------|
| 1 | JPEG reads from the stream | **Partly done.** A gray or RGB JPEG painted smaller than itself is decoded from its stream. The pass-through and full-size paths still read the file. |
| 2 | Raw-sample whole read | Done, but the plan's premise is wrong (see below). Bounded by the declared rows, not deleted. A fax stream is still read whole. |
| 3 | `decode_jbig2_image` | Done. Deleted; its caller goes through `decode_jbig2_bits`. |
| 4 | Indexed palette, CIDToGIDMap | Done. |
| 5 | `add_pdfa_identification` | Done. The row cannot show it (see below); an engine proof does. |
| 6 | `decode_slice` and `Decoder::decode` `#[cfg(test)]` | **Not done.** Blocked by existing tests and by files outside my share. |
| 7 | TSA reply cap | Done. |
| 8 | Interlaced PNG | Done. |

## Per site

### 1. JPEG, `extractors/images.rs:1283-1291`, `:2736` (`decode_dct_covering`)

Read whole: `ImageData::Jpeg(xobject.decode_stream_data()?)`, the compressed file, for every DCT image.

Change: when `extract_image_from_xobject_at` is called with a `target` and the image is a gray or RGB JPEG that
decodes at a reduction (`jpeg_decoder::Decoder::scale`), `decode_dct_covering` reads the header and the scans
from `decoded_reader` (`decoders/stream.rs:158`) and returns the reduced raster as `ImageData::Raw`, with its
own size in `jpx_decoded_dims` as a reduced JPX decode does. The file is never held. This is the same decoder
call `PdfImage::to_dynamic_image_covering` made on the bytes, so the pixels are the same.

What stays whole, and why: with no `target`, or a JPEG that needs no reduction, or a CMYK/YCCK JPEG,
`ImageData::Jpeg(bytes)` is the image, because the bytes are what `save_as_jpeg`, `to_base64_data_uri`,
`ffi.rs:2556`, `document.rs:2028/26136`, `separation_renderer.rs:3006`, `host/font_budget.rs:133` and
`host/images/execute.rs:362` (the CMYK decode) read. Changing the payload of `ImageData::Jpeg` would break every
one of those files, all outside my share. I did not change the type. The full-size decode needs the file in
memory in any case: the `image` crate's decoder takes a slice, and swapping decoders would change pixels.
So `extract_image_from_xobject_at#1` is still a site, and the lint will still flag it. Someone has to decide
whether the pass-through bytes are a table (the allow list names no such kind), or change `ImageData::Jpeg`.

Test: `engine_proofs/extract_jpeg_window.rs`. Before the fix:
`a_jpeg_painted_smaller_is_not_held_as_a_file` failed (`assertion failed: !held`) and
`a_jpeg_decodes_at_its_target_to_the_pixels_of_its_bytes` failed
(`gray at (24, 16): whether the file is kept`); the pixel assertions passed. The first holds a 14 MiB JPEG to a
4 MiB growth; the second compares the pixels at five targets with those of the file's bytes.

The memprobe's `hugeJpeg` rows are unchanged in effect: that JPEG is 1.5 MB, so the copy it saves is small.
`render hugeJpeg` peaks at 54,274,710 (limit 58,720,256), `reduceImages hugeJpeg` at 52,082,604.

### 2. Raw-sample read, `images.rs:1297-1304`

The plan says this is a whole read the row cursor replaced, to delete if nothing calls it. Something does:
`host/images/stream.rs:fill` (`:972`) and `host/images/execute.rs:458` hand the extractor one in-memory strip of
exactly `rows * row_bytes`, and the library's whole-image calls (`document.rs:25768`, `:26012`,
`separation_renderer.rs:2945`) take this branch too. So the branch is the row cursor's own strip decode, and
"route it through the row cursor" would be circular. I took the narrower choice: the branch reads
`read_decoded_window(xobject, declared)`, where `declared = ceil(width * ink_count * bits / 8) * height`, the
same size `RowGeometry` documents as "exactly the data it expects". A fax stream (`is_ccitt`) is the codec's
input, not samples, and is still read whole (`#2`).

Effect: the picture is unchanged; the stored `ImageData::Raw` buffer no longer carries the stray tail (it was
oversized before; the `image` crate ignored the excess).

Test: `mem_bounded_tables::a_raw_image_is_read_to_its_declared_rows` (8, 16 and 1 bit; 48 MiB of stray bytes
past the rows). Before: `8 bits: reading a 48 byte image grew the thread by 201333032 bytes`. The picture
assertion passed before, as it should: it pins the output.

### 3. `decode_jbig2_image`, `images.rs:1233-1240`, `:2713`

Deleted (and its `rendering`-off stub). The caller reads `decode_jbig2_bits` and `bilevel_to_gray` expands the
packed rows to the 8-bit gray the whole-image extraction stores (black 0, white 255). The 8-bit raster is kept
on purpose: `PdfImage` consumers (`save_raw_as_png` and others) assume 8-bit samples, so a packed 1-bit image
would break the library's export. A stub of `decode_jbig2_bits` for builds without `rendering` was added.

Behaviour that differs from the old decoder, and only for streams that decode to the wrong size: rows the
decoder did not reach are white, and rows are clipped to the declared width and height, where the old collector
kept whatever width it was pushed.

Test: `engine_proofs/extract_jbig2_gray.rs`. It is a pin of the output, not a failing-first test: the output is
unchanged by design, so it passes before and after. I show it can fail below.

### 4. Indexed palette and CIDToGIDMap

- `images.rs:1841` (`resolve_indexed_palette`): `read_decoded_window(&lookup_obj, (hival + 1) * N)`; a hival that
  is not 0..=255 leaves 256 entries, the most an 8-bit index names.
- `rendering/resolution/color.rs:617` (`resolve_indexed`): `read_decoded_window(&lookup, (min(hival, 255) + 1) * N)`.
  `N` is computed before the read now (it was after), with the same fallbacks.
- `fonts/font_dict.rs:1773` (`parse_descendant_fonts`): `read_decoded_window(&stream_obj, 131_072)`.

The helper is `decoders/stream.rs:158,178` (`decoded_reader`, `read_decoded_window`). **The plan names
`stream_reader()`; no such function exists in the tree.** The closest is `host::content::parts::decoded_reader`,
which I did not call from the extractors, so I wrote the two small functions in `decoders/stream.rs`, a file in my
share.

Output differences, for completeness. All are on streams that break the spec: a palette hival above 255 now
stops at 256 entries; a CIDToGIDMap stream of odd length longer than 131,072 bytes used to fall back to
Identity and now maps its first 65,536 entries; the map no longer carries entries past CID 65,535 (never read).

Tests: `mem_bounded_tables.rs`, three tests, each with 48 MiB of stray bytes behind the declared table and the
exact-length table as the control for the output. Before:

    an_indexed_image_palette_is_read_to_its_declared_length: reading a palette of 12 bytes grew the thread by 67243492 bytes
    an_indexed_paint_palette_is_read_to_its_declared_length: reading a palette of 12 bytes grew the thread by 67240416 bytes
    a_cid_to_gid_map_is_read_to_65536_entries: grew the thread by 117576437 bytes

(the CID map's first failure was `stray == exact`: the old map held 24 million stray entries.)

### 5. `add_pdfa_identification`, `compliance/converter.rs:513-560`, `:1633`, `:1663`

Read whole: the decoded packet (`decode_stream_data`, or `read_whole_raw` as the fallback), a lossy `String`
copy of it, and a patched `String` beside them.

Change: `splice_before_rdf_close` reads the packet twice as it decodes. The first pass reads XML events
(quick-xml, as `extractors::xmp` does) for the offset of the last `</rdf:RDF>` end tag and the length. The
second copies the packet in pieces around the identification block into a buffer of exactly the patched size.
The result is byte-identical to the old splice for a normal packet, and now byte-exact where the old one went
through lossy UTF-8. A packet with no RDF end tag, or that cannot be read, is rebuilt, as before.

Open choice: the patched packet is still held once. The editor stages a modified object in memory
(`modified_objects: HashMap<u32, Object>`), and nothing in the engine spills a stream, on the web least of all.
So 64 MiB of packet costs 64 MiB, not a window; that is one copy where there were three.

`add_pdfa_identification` is now `pub(crate)` (a visibility change between patch markers, like
`convert_with_editor`) so that the proof can stage it alone.

**The memprobe row cannot show this fix.** `convertToPdfA xmp64` peaks at 134,914,747 bytes on the old code and
on the new, to the byte. The whole conversion is dominated by something else: it validates, then commits with
`commit_in_place` (`save_to_bytes`, the whole file in memory, `editor/document_editor.rs:1548`), and the
validators read the packet too. All of that is outside my share. The row stays, as a guard on the operation
(limit 129 MiB, the measured peak rounded up, with that said in `limit()`), and it reads the output back: the
identification is in the committed packet, which is Flate-coded there. What fails before and passes after is the
staging step, measured alone:

Test: `engine_proofs/mem_pdfa_xmp.rs`. `a_64_mib_packet_is_identified_without_holding_it_as_text`. Before:
`identifying a 67092929 byte packet grew the thread by 134204042 bytes`; the bound is the packet plus 16 MiB.
`the_identification_is_spliced_before_the_last_rdf_close` pins the bytes for a plain and a Flate packet and the
rebuild; it passed before and after.

### 6. `decode_slice` and `Decoder::decode` — not done

Making them `#[cfg(test)]` breaks things I may not touch:

- Existing tests use them: `tests/test_decoders.rs` and `tests/test_text_extractor.rs` call
  `StreamDecoder::decode` and `decode_stream`, and `make test-rust` builds them. I may not edit an existing test
  to make it pass, so I stopped and recorded it here.
- Non-test callers outside my share: `object.rs:513` (the `Dictionary` arm decodes an empty slice through
  `decode_stream_with_params`) and `xref.rs:974` (the xref stream, a listed table).

`decode_slice#1` stays a site. To finish: a task that owns `object.rs` and `xref.rs` moves those two callers to
readers, and someone decides what happens to `tests/test_decoders.rs`.

### 7. TSA reply, `signatures/tsa_client.rs:27`, `:148-160`

`MAX_TSA_REPLY` is 1 MiB; the body is read with `.take(MAX_TSA_REPLY + 1)`, and a longer reply is
`Error::InvalidPdf("TSA reply exceeds 1048576 bytes")`.

Test: `engine_proofs/tsa_reply_cap.rs`, over a local TCP server that sends a reply of exactly the cap and one of a
byte more. It needs `--features tsa-client`, which is **not** in the plan's feature list nor in `build.json`, so
the package never builds this code; I ran it with the feature added (`/tmp/rt.sh`), and the file is
`cfg`-gated. For the same reason I wrote no changelog bullet for it. Before: `one byte past the window: Invalid
PDF: TSA response is not a valid TimeStampResp: unknown/unsupported ASN.1 DER tag: 0x00 at DER byte 1`.
My first server flaked once under the injected failure ("Peer disconnected"): it read the request once and
closed with bytes unread. It now reads the whole request first; five runs in a row pass.

### 8. Interlaced PNG, `host/image_embed.rs:151-637`

Read whole: `read_whole(reader, length)`, the entire file, into `ImageBytes::Whole`.

Change: `PngShape.interlaced`; `take` no longer reads a PNG whole. `Adam7` (`:593`) holds one `png::Reader` per
non-empty pass (`passes_of`, `:511`), each on its own `Position` (`:530`), a read cursor into the one shared
source, and each drops the rows of the earlier passes and never reads those after its own. For each output row,
`Adam7::next` pulls the next row of each pass that has a pixel in it and places the pixels. The `png` crate does
the unfiltering and `EXPAND`, so palette, `tRNS` and low depths come out as they did. A cancel polls inside the
skips as well.

**The plan says "seven 32 KiB windows plus a row each". Measured, it is more:** about 2.6 MB for a 1024-wide RGBA
image, of which 0.8 MB is what a non-interlaced PNG embed of the same width costs. The seven decoders cost a few hundred KiB each
(the `png` crate's own buffers); the cost stops growing at 2048 rows. It is bounded, not 32 KiB. The proof bounds
at 3 MiB.

Tests, in `engine_proofs/builder_image_stream.rs` (appended; no existing test touched):

- `an_interlaced_png_of_each_colour_type_embeds_the_pixels_and_the_alpha_of_todays_path`: every spec of
  `spec_set()` (grays, gray-alpha, RGB, RGBA, 1-16 bit, indexed with and without `tRNS`, keyed gray and RGB) at
  six sizes, including a pass with no pixel, a 1x1, and widths under a byte, with all five PNG row filters in
  rotation. It compares description, pixels and alpha with the whole-image path. It passed before and after.
- `an_interlaced_png_embeds_in_memory_that_does_not_grow_with_its_height`. Before (8 MiB file):
  `a 8851213 byte interlaced PNG took 40736976 bytes to embed`.

Row: `imagesToPdf interlacedPng`, 16 MiB, limit 16 MiB. **The plan names it `buildImage interlacedPng`; the
memprobe has no `buildImage` (it is the fuel probe's name), and its image op is `imagesToPdf`.** I used the
memprobe's name. Run against the old `image_embed.rs`: `imagesToPdf interlacedPng peak_over_baseline=80134082
limit=16777216 FAIL`. Now: `peak_over_baseline=2687203 ... ok`. I did not add a fuel-probe row.

## `whole_reads.txt`

`engine_proofs/whole_reads.rs` fails when a listed caller no longer reads whole, so I removed six lines from
`whole_reads.txt`: `add_pdfa_identification`, `decode_jbig2_image`, `resolve_indexed_palette`,
`parse_descendant_fonts`, `color::resolve_indexed` and `host::image_embed::take`. The plan
forbids editing the lint and `tool/whole_buffer_allow.txt`; I touched neither. Other tasks edit the same file at
the same time and will conflict on it; each line is its own.

## Injected failures

One break per fix, all at once, one rebuild; each test named the break it caught. Then every file was restored
from a copy made before and compared byte for byte, and no marker of a break is left.

| Break | Test that failed | Message |
|---|---|---|
| TSA: `if false && out.len() as u64 > MAX_TSA_REPLY` | `tsa_reply_cap::a_timestamp_reply_past_the_window_is_an_error` | `one byte past the window: Invalid PDF: TSA response is not a valid TimeStampResp: unknown/unsupported ASN.1 DER tag: 0x00 at DER byte 1` |
| image palette window `u64::MAX` | `an_indexed_image_palette_is_read_to_its_declared_length` | `reading a palette of 12 bytes grew the thread by 67243492 bytes` |
| paint palette window `u64::MAX` | `an_indexed_paint_palette_is_read_to_its_declared_length` | `reading a palette of 12 bytes grew the thread by 67240416 bytes` |
| CID map window `u64::MAX` | `a_cid_to_gid_map_is_read_to_65536_entries` | `reading a map of 65,536 entries grew the thread by 117576437 bytes` |
| raw window `u64::MAX` | `a_raw_image_is_read_to_its_declared_rows` | `8 bits: reading a 48 byte image grew the thread by 234887368 bytes` |
| `bilevel_to_gray` polarity swapped | `extract_jbig2_gray::a_jbig2_image_extracted_whole_is_its_bitmap_as_8_bit_gray` | `the bitmap as gray bytes` |
| `decode_dct_covering` always `None` | `extract_jpeg_window::a_jpeg_painted_smaller_is_not_held_as_a_file` and `a_jpeg_decodes_at_its_target_to_the_pixels_of_its_bytes` | `assertion failed: !held`; `gray at (24, 16): whether the file is kept` |
| XMP offset `at + 1` | `mem_pdfa_xmp::the_identification_is_spliced_before_the_last_rdf_close` | `assertion left == right failed: plain` |
| XMP buffer reserve `0` | `mem_pdfa_xmp::a_64_mib_packet_is_identified_without_holding_it_as_text` | `assertion failed: got.ends_with(b"</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end='w'?>")` (the offset break reached it first; the reserve break is not told apart) |
| Adam7 row test `% dy == 1` | `an_interlaced_png_of_each_colour_type_...` and the existing `an_interlaced_png_embeds_as_it_does_today` | `gray8 37x29: the same pixels`; `assertion left == right failed: the same pixels` |
| `PASS_WINDOW` 4 MiB | `an_interlaced_png_embeds_in_memory_that_does_not_grow_with_its_height` | `a 17702596 byte interlaced PNG took 31818169 bytes to embed` |

Not told apart: the XMP reserve break. Both XMP breaks were applied together, and the offset break failed the
64 MiB test on its `ends_with` before the growth assertion ran. The growth assertion is shown to fail by the
unmodified old code instead (134,204,042 bytes, above).

## Summary lines

    Rust lib, plan's features (cargo nextest run --lib --features "icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract,test-support,public-api,cjk-form-fonts"):
        Summary [ 522.993s] 6763 tests run: 6763 passed (7 slow), 5 skipped   (the final tree)
    The same suite before the last test-only edits (doc-comment order, TSA server):
        Summary [ 583.582s] 6763 tests run: 6763 passed (7 slow), 5 skipped
    tsa_reply_cap (needs --features tsa-client): test result: ok. 1 passed; 0 failed (five runs)
    bash tool/check_patch_markers.sh: exit 0
    bash tool/check_fork_tests.sh: exit 0
    bash tool/check_cancel_polls.sh: every loop over a document-scale collection polls for a cancel (exit 0)
    bash tool/check_changelog_shape.sh: exit 0
    dart analyze --fatal-infos lib hook test bin: No issues found!
    make test-memory DART=dart: exit 0
        render hugeJpeg            peak_over_baseline=54274710  limit=58720256  ok
        extractImages hugeJpeg     peak_over_baseline=623084    limit=16777216  ok
        reduceImages hugeJpeg      peak_over_baseline=52082604  limit=58720256  ok
        render hugeJbig2           peak_over_baseline=102425288 limit=102760448 ok
        extractImages hugeJbig2    peak_over_baseline=100251490 limit=100663296 ok
        imagesToPdf interlacedPng  peak_over_baseline=2687203   limit=16777216  ok   (new)
        convertToPdfA xmp64        peak_over_baseline=134914747 limit=135266304 ok   (new)
    make test-ops-native DART=dart: 01:15 +666 ~1: All tests passed!  (exit 0)
    make test-fuel DART=dart: exit 2, one row: saveFull tagged  max_gap 61522606  total 977069821  FAIL: total below 1000000000
        The same row fails on the unmodified fork (stashed): total 966758908. It was failing before my work.
    cargo check --lib --features "icc,legacy-crypto,signatures,pdfa,extract,native-bridge" (no rendering): Finished
        (default features alone do not build: the host layer needs native-bridge; that was so before.)

## Choices where the plan left one open

1. `stream_reader()` does not exist; I wrote `decoded_reader` and `read_decoded_window` in `decoders/stream.rs`.
2. Item 1 is partial (above), because `ImageData::Jpeg` is a public type used by files outside my share.
3. Item 2 bounds the read by the declared rows instead of deleting it or routing it through a cursor.
4. Item 3 keeps the 8-bit raster the extraction stores, but builds it from the 1-bit decode.
5. Item 4: hival above 255 is read to 256 entries; an unusable hival reads 256 entries.
6. Item 5 holds the patched packet once; the editor has nowhere to put it.
7. Item 6 is left, with its blockers named.
8. Item 8's memory is a few hundred KiB per pass reader, not 32 KiB. The memprobe row is `imagesToPdf`, not
   `buildImage`.
9. The changelog has bullets for the five changes a user of the package can see. None for the TSA cap, which the
   package does not build, nor for the JBIG2 reroute, which changes no output.
10. `mem_bounded_tables.rs`, `mem_pdfa_xmp.rs`, `extract_jpeg_window.rs`, `extract_jbig2_gray.rs` and
    `tsa_reply_cap.rs` are new proof files in `engine_proofs/` (each test says why no Dart battery sees it).
    `builder_image_stream.rs` got two tests appended.

## Not done, and who has to

- Item 6 as above.
- The JPEG pass-through (`extract_image_from_xobject_at#1`) and the fax stream (`#2`) remain flagged. The lint
  needs a ruling on them, or a change to `ImageData::Jpeg`.
- `convertToPdfA` stays whole-document at its commit and in its validators, so no row can hold it to the
  ceiling until `commit_in_place` and the PDF/A validators read in pieces.
- `make test-fuel`'s `saveFull tagged` fixture is under its 10^9 floor on the unmodified fork.
