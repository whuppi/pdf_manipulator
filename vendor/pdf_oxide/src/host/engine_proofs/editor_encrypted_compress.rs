//! A full save with encryption deflates every stream the way an unencrypted
//! one does, whatever its size: a large compressible stream of an encrypted
//! document comes out smaller, opens with the password and decodes to the
//! bytes it held.

use super::support::{contains, pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, EncryptionAlgorithm, EncryptionConfig, SaveOptions};
use crate::host::deflate::MEMORY_STREAM;
use crate::object::{Object, ObjectRef};
use std::collections::HashMap;

const PASSWORD: &str = "secret";
const NOTE: &[u8] = b"a string in the stream's dictionary";

/// Bytes that compress well but not to nothing.
fn text(len: usize, salt: usize) -> Vec<u8> {
    let words = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf"];
    let mut out = Vec::with_capacity(len + 8);
    let mut i = salt;
    while out.len() < len {
        out.extend_from_slice(words[(i * 7 + i / 3) % words.len()].as_bytes());
        out.push(b' ');
        i += 1;
    }
    out.truncate(len);
    out
}

/// Bytes that do not compress.
fn noise(len: usize) -> Vec<u8> {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

/// The streams the catalog reaches, by object number: a large one over
/// `MEMORY_STREAM` (4), a small one under it (5), a large one that does not
/// compress (6), and a large one the save finds staged (7).
fn streams() -> [(u32, Vec<u8>); 4] {
    let large = MEMORY_STREAM as usize * 4;
    [
        (4, text(large, 0)),
        (5, text(MEMORY_STREAM as usize / 4, 1)),
        (6, noise(large)),
        (7, text(large, 2)),
    ]
}

fn note_dict() -> String {
    format!("/Note ({})", String::from_utf8_lossy(NOTE))
}

fn plain() -> Vec<u8> {
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /A 4 0 R /B 5 0 R /C 6 0 R /D 7 0 R >>\nendobj\n"
            .to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n".to_vec(),
    ];
    for (id, data) in streams() {
        objects.push(stream_object(id, &note_dict(), &data));
    }
    pdf_from_objects(&objects)
}

fn config(user: &str, algorithm: EncryptionAlgorithm) -> EncryptionConfig {
    EncryptionConfig::new(user, "owner").with_algorithm(algorithm)
}

/// The file `plain` makes with its streams as written, encrypted under an
/// empty user password.
fn encrypted(algorithm: EncryptionAlgorithm) -> Vec<u8> {
    let options = SaveOptions { compress: false, ..SaveOptions::with_encryption(config("", algorithm)) };
    DocumentEditor::from_bytes(plain())
        .unwrap()
        .save_to_bytes_with_options(options)
        .unwrap()
}

/// `source` saved with `options`, object 7 staged as the same data, the way an
/// edit stages a stream.
fn compressed(source: &[u8], options: SaveOptions) -> Vec<u8> {
    let mut editor = DocumentEditor::from_bytes(source.to_vec()).unwrap();
    let staged = streams()[3].1.clone();
    let mut dict = HashMap::new();
    dict.insert("Note".to_string(), Object::String(NOTE.to_vec()));
    editor.insert_modified(7, Object::stream(dict, staged));
    editor.save_to_bytes_with_options(options).unwrap()
}

/// Whether the /Length of stream `id` in `output` ends its data where
/// `endstream` begins; a reader that scans for the keyword never shows it.
fn length_holds(output: &[u8], doc: &PdfDocument, id: u32) -> bool {
    let head = format!("{id} 0 obj\n").into_bytes();
    let Some(at) = output.windows(head.len()).position(|w| w == head.as_slice()) else { return false };
    let rest = &output[at..];
    let Some(data_at) = rest.windows(7).position(|w| w == b"stream\n").map(|at| at + 7) else {
        return false;
    };
    let dict = String::from_utf8_lossy(&rest[..data_at]).into_owned();
    let Some(value) = dict.split("/Length ").nth(1) else { return false };
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    let Ok(number) = digits.parse::<usize>() else { return false };
    let length = if value[digits.len()..].starts_with(" 0 R") {
        let Ok(Object::Integer(n)) = doc.load_object(ObjectRef::new(number as u32, 0)) else {
            return false;
        };
        n as usize
    } else {
        number
    };
    rest.get(data_at + length..).is_some_and(|tail| tail.starts_with(b"\nendstream"))
}

fn check(output: &[u8], source_len: usize, label: &str) {
    assert!(
        output.len() < source_len / 2,
        "{label}: {} bytes from a source of {source_len}",
        output.len()
    );
    assert!(!contains(output, NOTE), "{label}: a dictionary string is in the file as it was");
    let doc = PdfDocument::from_bytes(output.to_vec()).unwrap();
    assert!(doc.is_encrypted(), "{label}: the output is not encrypted");
    assert!(doc.authenticate(PASSWORD.as_bytes()).unwrap(), "{label}: the password does not open it");
    for (id, data) in streams() {
        let object = doc.load_object(ObjectRef::new(id, 0)).unwrap();
        let Object::Stream { dict, .. } = &object else { panic!("{label}: object {id} is not a stream") };
        assert_eq!(
            dict.get("Note"),
            Some(&Object::String(NOTE.to_vec())),
            "{label}: object {id}'s dictionary string"
        );
        assert_eq!(object.decode_stream_data().unwrap(), data, "{label}: object {id}'s data");
        assert!(length_holds(output, &doc, id), "{label}: object {id}'s /Length");
        if id != 6 {
            assert_eq!(
                dict.get("Filter"),
                Some(&Object::Name("FlateDecode".to_string())),
                "{label}: object {id} is not deflated"
            );
        }
    }
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a deflated stream of an encrypted file from one copied as it was; only the size of the file shows it.
#[test]
fn compress_deflates_large_streams_under_aes_256() {
    let source = encrypted(EncryptionAlgorithm::Aes256);
    let output = compressed(
        &source,
        SaveOptions::with_encryption(config(PASSWORD, EncryptionAlgorithm::Aes256)),
    );
    check(&output, source.len(), "AES-256");
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a deflated stream of an encrypted file from one copied as it was; only the size of the file shows it.
#[test]
fn compress_deflates_large_streams_under_rc4() {
    let source = encrypted(EncryptionAlgorithm::Rc4_128);
    let output = compressed(
        &source,
        SaveOptions::with_encryption(config(PASSWORD, EncryptionAlgorithm::Rc4_128)),
    );
    check(&output, source.len(), "RC4-128");
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a deflated stream of an encrypted file from one copied as it was; only the size of the file shows it.
#[test]
fn compress_deflates_large_streams_when_the_source_encryption_is_kept() {
    let source = encrypted(EncryptionAlgorithm::Aes256);
    let output = compressed(
        &source,
        SaveOptions { keep_source_encryption: true, ..SaveOptions::full_rewrite() },
    );
    let doc = PdfDocument::from_bytes(output.clone()).unwrap();
    assert!(doc.is_encrypted());
    assert!(output.len() < source.len() / 2, "{} bytes from a source of {}", output.len(), source.len());
    for (id, data) in streams() {
        let object = doc.load_object(ObjectRef::new(id, 0)).unwrap();
        assert_eq!(object.decode_stream_data().unwrap(), data, "object {id}");
    }
}
