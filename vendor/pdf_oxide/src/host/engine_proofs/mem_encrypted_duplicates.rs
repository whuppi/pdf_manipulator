//! Identical images of an encrypted source merge however large they are, and
//! finding them decrypts through a reader, never into a buffer. Each proof
//! runs in a process of its own (`own_process`): they hold megabytes, and
//! one reads the allocator counter.

use super::filter_readers::{HELD, PEAK};
use super::own_process::alone;
use super::support::{pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, EncryptionConfig, SaveOptions};
use crate::host::deflate::MEMORY_STREAM;
use crate::host::duplicates::{find, same_stream_data, Origin};
use crate::object::Object;
use std::sync::atomic::Ordering;

const IMAGE: &str = "/Type /XObject /Subtype /Image /Width 1024 /Height 1024 \
                     /BitsPerComponent 8 /ColorSpace /DeviceGray";
const MIB: usize = 1024 * 1024;

/// Bytes that do not compress, so a re-deflate never changes a stream.
fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64 ^ seed;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

/// A one-page file whose page shows two images, `first` and `second`.
fn plain(first: &[u8], second: &[u8]) -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] \
          /Resources << /XObject << /A 4 0 R /B 5 0 R >> >> >>\nendobj\n"
            .to_vec(),
        stream_object(4, IMAGE, first),
        stream_object(5, IMAGE, second),
    ])
}

/// The file `plain` makes, written with AES-256 and an empty user password.
fn encrypted(first: &[u8], second: &[u8]) -> Vec<u8> {
    let mut editor = DocumentEditor::from_bytes(plain(first, second)).unwrap();
    editor
        .save_to_bytes_with_options(SaveOptions::with_encryption(EncryptionConfig::new("", "owner")))
        .unwrap()
}

fn images(doc: &PdfDocument) -> usize {
    doc.all_object_ids()
        .into_iter()
        .filter(|&id| id != 0 && !doc.is_free_entry(id))
        .filter_map(|id| doc.load_object(crate::object::ObjectRef::new(id, 0)).ok())
        .filter(|obj| {
            matches!(obj, Object::Stream { dict, .. }
                if dict.get("Subtype").and_then(|s| s.as_name()) == Some("Image"))
        })
        .count()
}

fn save(source: &[u8], options: SaveOptions) -> Vec<u8> {
    DocumentEditor::from_bytes(source.to_vec())
        .unwrap()
        .save_to_bytes_with_options(options)
        .unwrap()
}

fn merging() -> SaveOptions {
    SaveOptions { keep_source_encryption: false, merge_duplicates: true, ..SaveOptions::full_rewrite() }
}

/// Two identical images of 1 MiB each, above the 256 KiB `MEMORY_STREAM` that
/// used to stop an encrypted source from merging, are written once when the
/// source is encrypted, and the compressing save is a megabyte smaller than
/// the one that keeps both. No Dart battery can observe this: Dart reads the
/// saved file through its API, which shows the same page whether the file
/// holds one copy of the image or two, and has no way to read the object
/// graph.
#[test]
fn identical_large_images_of_an_encrypted_source_merge_and_the_file_shrinks() {
    alone(module_path!(), "identical_large_images_of_an_encrypted_source_merge_and_the_file_shrinks", || {
        let data = noise(MIB, 41);
        assert!(data.len() as u64 > MEMORY_STREAM);
        let source = encrypted(&data, &data);

        let merged = save(&source, merging());
        let kept = save(&source, SaveOptions { merge_duplicates: false, ..merging() });
        assert_eq!(images(&PdfDocument::from_bytes(merged.clone()).unwrap()), 1);
        assert_eq!(images(&PdfDocument::from_bytes(kept.clone()).unwrap()), 2);
        assert!(
            merged.len() + MIB - 4096 < kept.len(),
            "merged {} bytes against {} bytes that keep both",
            merged.len(),
            kept.len()
        );

        // The output stays encrypted when the source's encryption is kept, and the
        // two copies are one there too.
        let still_locked = save(&source, SaveOptions { keep_source_encryption: true, ..merging() });
        assert_eq!(images(&PdfDocument::from_bytes(still_locked).unwrap()), 1);
    });
}

/// Two large images of an encrypted source that have one length and different
/// bytes stay two: the digest and the side-by-side comparison read all of
/// both. No Dart battery can observe this: it is a count of image objects in
/// the saved file.
#[test]
fn large_encrypted_images_that_differ_stay_two() {
    alone(module_path!(), "large_encrypted_images_that_differ_stay_two", || {
        let (a, mut b) = (noise(MIB, 42), noise(MIB, 42));
        let last = b.len() - 1;
        b[last] ^= 1;
        let source = encrypted(&a, &b);
        let merged = save(&source, merging());
        assert_eq!(images(&PdfDocument::from_bytes(merged).unwrap()), 2);
    });
}

/// Finding the twins of an encrypted source reads each stream through a
/// decrypting reader: two images of 3 MiB are found equal with the process
/// growing by less than one of them, where a whole decrypt of either held 3
/// MiB at least. No Dart battery can observe this: the allocator counter is the
/// engine's own, and the file saved is the same bytes either way. It runs in
/// a process of its own, since the counter counts every thread.
#[test]
fn finding_the_twins_of_an_encrypted_source_decrypts_through_a_reader() {
    alone(module_path!(), "finding_the_twins_of_an_encrypted_source_decrypts_through_a_reader", || {
        let data = noise(3 * MIB, 43);
        let source = encrypted(&data, &data);
        drop(data);
        let doc = PdfDocument::from_bytes(source).unwrap();
        assert!(doc.is_encrypted());
        let ids: Vec<u32> = doc
            .all_object_ids()
            .into_iter()
            .filter(|&id| id != 0 && !doc.is_free_entry(id))
            .collect();
        let origins = [Origin { doc: &doc, objects: ids.iter().map(|&id| (id, id)).collect(), staged: None }];

        let base = HELD.load(Ordering::Relaxed);
        PEAK.store(base, Ordering::Relaxed);
        let aliases = find(&origins, &CancelCheck::none()).unwrap();
        let growth = PEAK.load(Ordering::Relaxed).saturating_sub(base);

        assert_eq!(aliases.len(), 1, "one of the two equal images stands for both: {aliases:?}");
        assert!(growth < MIB, "finding the twins of two 3 MiB streams grew the process by {growth} bytes");
    });
}

fn origin(doc: &PdfDocument) -> Origin<'_> {
    Origin { doc, objects: Vec::new(), staged: None }
}

/// Two streams of an encrypted source are the same when their decrypted bytes
/// are, and not when the last byte differs or one is a byte longer. No Dart
/// battery can observe this: SHA-256 does not collide on any file a battery
/// could write, so the digests the merge goes by never meet in such a case and
/// only the comparison itself, called on streams of one length, tells them
/// apart.
#[test]
fn two_encrypted_streams_are_the_same_only_when_every_decrypted_byte_is() {
    alone(module_path!(), "two_encrypted_streams_are_the_same_only_when_every_decrypted_byte_is", || {
        let data = noise(MIB, 44);
        let mut last_differs = data.clone();
        *last_differs.last_mut().unwrap() ^= 1;
        let longer = [data.clone(), vec![0u8]].concat();
        // Streams 4 and 5 of each file.
        let equal = PdfDocument::from_bytes(encrypted(&data, &data)).unwrap();
        let last = PdfDocument::from_bytes(encrypted(&data, &last_differs)).unwrap();
        let long = PdfDocument::from_bytes(encrypted(&data, &longer)).unwrap();

        let same = |doc: &PdfDocument| same_stream_data(&origin(doc), 4, &origin(doc), 5, &crate::cancel::CancelCheck::none()).unwrap();
        assert!(same(&equal), "equal streams");
        assert!(!same(&last), "the last byte differs");
        assert!(!same(&long), "one stream is a byte longer");
        // The first streams of two files are equal data, each under its own key.
        assert!(same_stream_data(&origin(&equal), 4, &origin(&last), 4, &crate::cancel::CancelCheck::none()).unwrap());
    });
}
