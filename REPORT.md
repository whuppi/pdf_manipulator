# t-578 — an encrypted full save recompresses a stream over 256 KiB

Gate result: everything passes except `make test-fuel`, which fails the same way without my change (below).

## Cause

`vendor/pdf_oxide/src/editor/document_editor.rs`, `write_stream_copy` (`if encrypting_output { return Ok(false); }`, near :3925) refused every stream when the output was encrypted. The staged path (near :8159) matched only `(deflate, None, Object::Stream)`, so it also skipped encrypted output. `write_stream_body` documented "Unencrypted output only". An encrypted full save, and so `compress` of an encrypted document, therefore went through the serializer, which copies a stream's bytes. The `MEMORY_STREAM` guard in `deflate_stream` also left any stream over 256 KiB untouched.

## Change (document_editor.rs; see t-578-pdf_oxide.patch)

- `write_source_stream`, `write_merged_stream` and `write_stream_copy`: the `encrypting_output: bool` parameter is now `handler: &Option<EncryptionWriteHandler>`. The `return Ok(false)` for encrypted output is gone. The five call sites pass `&encryption_handler`.
- `write_stream_body` takes `handler`. The count pass still works on the plain data. The body goes through `handler.stream_cipher(out_id, 0).encrypting(writer, iv)`:
  - Copy and Memory write a direct `/Length` of `cipher.encrypted_len(..)`.
  - Streamed deflates into the encrypting writer and writes an indirect `/Length` object after, measured as the writer's position delta.
  - Strings in the stream dictionary are enciphered with `encrypt_string(.., out_id, 0)`, as `write_dictionary_encrypted` does.
  - Without a handler the cipher is `Cipher::Identity`, a pass-through.
- The staged path no longer requires `None` for the handler and passes `&encryption_handler`. Stale comments (`deflate_stream`, the doc comments) were updated.
- New proof file `src/host/engine_proofs/editor_encrypted_compress.rs`, registered in `engine_proofs/mod.rs`.
- `CHANGELOG.pre.md` (t-578-pkg.patch): one Fixed bullet in 6.0.0-dev.0.

## Tests

Three tests in `editor_encrypted_compress`. The fixture holds streams 4, 5, 6 and 7:
- 4: 1 MiB of compressible text.
- 5: 64 KiB of text (the in-memory case).
- 6: 1 MiB of noise.
- 7: staged through `insert_modified`, so it takes the staged path.

Each stream carries a string in its dictionary. The fixture is encrypted with `compress:false`, then compressed.

- `compress_deflates_large_streams_under_aes_256` and `..._under_rc4` (Rc4_128): the output is under half the source size, is encrypted, opens with the password, and every stream decodes to the same bytes. Each stream's dictionary string round-trips and is absent from the file as plain text, and each stream's `/Length` lands on `endstream`.
- `..._when_the_source_encryption_is_kept` (`keep_source_encryption`): size and decode.

**Before the fix** (all 3 fail on size):
- `RC4-128: 3147323 bytes from a source of 3212509`
- `AES-256: 3148031 bytes from a source of 3213223`
- kept: `3148031 bytes from a source of 3213223` (assert message `{} bytes from a source of {}`)

**After:** 3 passed.

### Injected failures (each restored; `cmp` against the saved fixed file, and `git diff` shows only the intended change)

- Dictionary strings left plain: `AES-256: a dictionary string is in the file as it was`, and the same for RC4. (The first version of the proof caught only RC4, because AES decrypt of a plain string falls back to the raw bytes. I added the in-file check.)
- In-memory `/Length` without the cipher's padding: `AES-256: object 5's /Length`. (Missed by the first version, because the reader scans for `endstream`. I added `length_holds`.)
- Staged path excludes encrypted output again: all 3 fail, e.g. `AES-256: 2103628 bytes from a source of 3213223`.
- Wrong object key (generation 1): RC4 fails (`RC4-128: 2168116 bytes from a source of 3212509`). AES-256 passes because it derives no per-object key.

### Memprobe

`compress encrypted` and `fullRewriteSave encrypted` already existed, with the limit at 16 MiB. They pass before and after, so no row could be made to fail first. The fixture is 60 MiB of noise, so the streamed-encrypted deflate path is not hit there; it keeps its Copy. The proof above holds that path. `compress encrypted wall_ms=2` looks short, but the row's closure succeeded and the result is unchanged by this patch.

## Summary lines

- Rust suite: `Summary [ 374.252s] 6759 tests run: 6759 passed (5 slow), 5 skipped`
- `make test-memory`: `test result: ok. 7 passed; 0 failed`, then 71 rows `ok`; `fullRewriteSave encrypted peak_over_baseline=932570 limit=16777216 ok`; `compress encrypted peak_over_baseline=947069 limit=16777216 ok`
- `make test-ops-native`: `All tests passed!` (+666 ~1)
- `dart analyze --fatal-infos lib hook test bin`: `No issues found!`
- `check_patch_markers.sh`, `check_fork_tests.sh`, `check_cancel_polls.sh`: exit 0 (the last prints "every loop over a document-scale collection polls for a cancel")
- `check_changelog_shape.sh`: exit 0
- **`make test-fuel`: FAILS, not caused by this change.** `saveFull tagged 61748893 976533584 16890 FAIL: total below 1000000000`. On the unchanged fork (my change stashed) it fails identically: `saveFull tagged 61770066 967902793 16890 FAIL: total below 1000000000`. The fixture's total fuel is under the 10^9 floor. The fix is in `tool/fuelprobe`, which is not my file, so I left it.

## Choices where the plan left one open

- Strings in the stream dictionary are enciphered inside `write_stream_body` (a small local function), because the serializer's `write_dictionary_encrypted` is private and `object_serializer.rs` is not in my files.
- The exemption for `/Crypt /Identity` is not applied on write; the serializer does not apply it either. ObjStm/XRef streams are still excluded from the copy.
- Only the AES-256 and RC4-128 algorithms are tested, as named. A third test covers `keep_source_encryption`.
- The existing t-572 bullet in `CHANGELOG.pre.md` still says "(an encrypted save writes such a stream as it is)". That is now untrue. I left it as it is (the plan asks for one new bullet), so the maintainer should trim it.
- No memprobe row was added (the existing two cover the plan's item 3).
- `PLAN.md` is untouched; `office_oxide` and `tiny_skia` are unchanged, so there is no patch for them.
