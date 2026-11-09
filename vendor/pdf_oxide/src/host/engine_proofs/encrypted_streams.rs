//! The data of an encrypted document's stream decrypts itself, in pieces.

use super::support::{pdf_from_objects, stream_object};
use crate::crypto::{active, AesKeySize, Padding};
use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, EncryptionAlgorithm, EncryptionConfig, SaveOptions};
use crate::encryption::stream::{from_io, is_exempt, Cipher};
use crate::error::Error;
use crate::object::{Extent, Object, ObjectRef, StreamBody};
use crate::writer::ObjectSerializer;
use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;

const ALGORITHMS: [EncryptionAlgorithm; 4] = [
    EncryptionAlgorithm::Rc4_40,
    EncryptionAlgorithm::Rc4_128,
    EncryptionAlgorithm::Aes128,
    EncryptionAlgorithm::Aes256,
];

/// Lengths that put the last block empty, partial and full, and one far past a chunk.
const LENGTHS: [usize; 7] = [0, 1, 15, 16, 17, 1000, 70_000];

fn plaintext(index: usize) -> Vec<u8> {
    (0..LENGTHS[index]).map(|b| (b as u8).wrapping_mul(7).wrapping_add(index as u8)).collect()
}

/// A plain source whose objects `4..` are the streams of `LENGTHS`, all reached from the catalog.
fn plain_source() -> Vec<u8> {
    let refs: String = (0..LENGTHS.len()).map(|i| format!("{} 0 R ", 4 + i)).collect();
    let mut objects = vec![
        format!("1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Streams [{refs}] >>\nendobj\n").into_bytes(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n".to_vec(),
    ];
    for i in 0..LENGTHS.len() {
        objects.push(stream_object(4 + i as u32, "", &plaintext(i)));
    }
    pdf_from_objects(&objects)
}

/// `source` saved under `algorithm` with `user` as the user password and its streams left uncompressed.
fn encrypted_for(source: Vec<u8>, algorithm: EncryptionAlgorithm, user: &str) -> Vec<u8> {
    let config = EncryptionConfig::new(user, "owner").with_algorithm(algorithm);
    DocumentEditor::from_bytes(source)
        .unwrap()
        .save_to_bytes_with_options(SaveOptions {
            compress: false,
            ..SaveOptions::with_encryption(config)
        })
        .unwrap()
}

fn encrypted(source: Vec<u8>, algorithm: EncryptionAlgorithm) -> Vec<u8> {
    encrypted_for(source, algorithm, "user")
}

fn extent_of(doc: &PdfDocument, id: u32) -> (HashMap<String, Object>, Extent) {
    match doc.load_object(ObjectRef::new(id, 0)).unwrap() {
        Object::Stream { dict, body: StreamBody::Extent(extent) } => (dict, extent),
        other => panic!("object {id}: {other:?}"),
    }
}

/// A reader that hands out at most `piece` bytes per read.
struct Pieces<R>(R, usize);

impl<R: Read> Read for Pieces<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(self.1);
        self.0.read(&mut buf[..n])
    }
}

/// No Dart battery sees it: it opens each algorithm's file and reads each stream back by object number, and the RC4 or AES key is derived per object inside the engine.
#[test]
fn every_algorithm_decrypts_every_stream_and_an_extent_made_before_the_password_reads_after_it() {
    for algorithm in ALGORITHMS {
        let doc = PdfDocument::from_bytes(encrypted(plain_source(), algorithm)).unwrap();
        let before: Vec<_> = (0..LENGTHS.len()).map(|i| extent_of(&doc, 4 + i as u32).1).collect();
        assert!(
            StreamBody::Extent(before[1].clone()).read_whole_raw().is_err(),
            "{algorithm:?}: a stream reads before the password is known"
        );
        assert!(doc.authenticate(b"user").unwrap(), "{algorithm:?}");
        for (i, extent) in before.into_iter().enumerate() {
            let body = StreamBody::Extent(extent);
            assert_eq!(body.read_whole_raw().unwrap(), plaintext(i), "{algorithm:?} stream {i}");
            assert_eq!(body.decrypted_len().unwrap(), LENGTHS[i] as u64, "{algorithm:?} stream {i}");
        }
    }
}

/// No Dart battery sees it: it feeds the decrypting reader a source that answers in 1 to 65536 bytes a read, which no Dart source does.
#[test]
fn every_chunk_size_decrypts_to_what_decrypt_stream_returns() {
    for algorithm in ALGORITHMS {
        let doc = PdfDocument::from_bytes(encrypted(plain_source(), algorithm)).unwrap();
        assert!(doc.authenticate(b"user").unwrap());
        for i in 0..LENGTHS.len() {
            let id = 4 + i as u32;
            let (_, extent) = extent_of(&doc, id);
            let handler = extent.source().handler().unwrap();
            let mut ciphertext = Vec::new();
            extent.stored_reader().read_to_end(&mut ciphertext).unwrap();
            let expected = handler.decrypt_stream(&ciphertext, id, 0).unwrap();
            for piece in [1, 2, 3, 5, 15, 16, 17, 63, 64, 1000, 4096, 65_535, 65_536] {
                let cipher = handler.cipher_for(id, 0).unwrap();
                let mut got = Vec::new();
                cipher
                    .decrypting(Pieces(&ciphertext[..], piece))
                    .unwrap()
                    .read_to_end(&mut got)
                    .unwrap();
                assert_eq!(got, expected, "{algorithm:?} stream {i} in pieces of {piece}");
            }
        }
    }
}

/// No Dart battery sees it: it hands the reader ciphertext whose last block carries a wrong pad, and Dart cannot forge a ciphertext.
#[test]
fn a_tampered_pad_is_a_typed_error() {
    let key = vec![0x42u8; 32];
    let iv = [0x24u8; 16];
    let sym = active().symmetric();
    for pad in [0u8, 17, 2] {
        // One block whose last byte is `pad` but whose neighbour is not: invalid under PKCS#7.
        let mut block = vec![9u8; 16];
        block[15] = pad;
        let body = sym.aes_cbc_encrypt(AesKeySize::Aes256, &key, &iv, &block, Padding::None).unwrap();
        let ciphertext = [iv.to_vec(), body].concat();
        let cipher = Cipher::Aes { size: AesKeySize::Aes256, key: key.clone() };
        let mut out = Vec::new();
        let err = cipher
            .decrypting(&ciphertext[..])
            .unwrap()
            .read_to_end(&mut out)
            .map(|_| ())
            .map_err(from_io)
            .unwrap_err();
        assert!(
            matches!(&err, Error::InvalidPdf(m) if m.contains("padding")),
            "pad {pad}: {err:?}"
        );
    }
    let cipher = Cipher::Aes { size: AesKeySize::Aes256, key };
    let short = cipher.decrypting(&[0u8; 7][..]).unwrap().read_to_end(&mut Vec::new());
    assert!(matches!(short.map_err(from_io), Err(Error::InvalidPdf(m)) if m.contains("too short")));
    let ragged = cipher.decrypting(&[0u8; 16 + 20][..]).unwrap().read_to_end(&mut Vec::new());
    assert!(matches!(ragged.map_err(from_io), Err(Error::InvalidPdf(m)) if m.contains("whole number")));
}

/// No Dart battery sees it: the stream is stored in the clear inside an encrypted file, a layout only a hand-built extent produces.
#[test]
fn a_crypt_identity_stream_an_object_stream_and_a_cross_reference_stream_are_read_as_stored() {
    let doc = PdfDocument::from_bytes(encrypted(plain_source(), EncryptionAlgorithm::Aes256)).unwrap();
    assert!(doc.authenticate(b"user").unwrap());
    let (_, any) = extent_of(&doc, 4);
    let source = Arc::clone(any.source());
    let mut head = vec![0u8; 8];
    assert_eq!(source.read_at(0, &mut head).unwrap(), 8);

    let name = |n: &str| Object::Name(n.to_string());
    let crypt = |parms: Object| {
        HashMap::from([
            ("Filter".to_string(), Object::Array(vec![name("Crypt")])),
            ("DecodeParms".to_string(), Object::Array(vec![parms])),
        ])
    };
    let parms = |entry: Option<&str>| {
        Object::Dictionary(entry.map(|n| ("Name".to_string(), name(n))).into_iter().collect())
    };
    let window = |dict: &HashMap<String, Object>| {
        StreamBody::Extent(Extent::new(Arc::clone(&source), 0, 8, ObjectRef::new(99, 0), dict))
    };

    for (what, dict, exempt) in [
        ("/Crypt /Identity", crypt(parms(Some("Identity"))), true),
        ("/Crypt with no name", crypt(parms(None)), true),
        ("/Crypt /StdCF", crypt(parms(Some("StdCF"))), false),
        ("/Type /ObjStm", HashMap::from([("Type".to_string(), name("ObjStm"))]), true),
        ("/Type /XRef", HashMap::from([("Type".to_string(), name("XRef"))]), true),
        ("an ordinary stream", HashMap::new(), false),
    ] {
        assert_eq!(is_exempt(&dict), exempt, "{what}");
        let read = window(&dict).read_whole_raw();
        if exempt {
            assert_eq!(read.unwrap(), head, "{what}: read as stored");
        } else {
            assert!(read.is_err(), "{what}: eight clear bytes are not an AES stream");
        }
    }
}

/// No Dart battery sees it: the saved file is compared with the plain bytes after the engine decrypted each stream while writing, which a Dart read of the file cannot separate from the engine's own decryption.
#[test]
fn an_encrypted_source_written_plain_carries_the_decrypted_data_and_its_length() {
    let serializer = ObjectSerializer::compact();
    for algorithm in ALGORITHMS {
        let doc = PdfDocument::from_bytes(encrypted(plain_source(), algorithm)).unwrap();
        assert!(doc.authenticate(b"user").unwrap());
        for i in 0..LENGTHS.len() {
            let object = doc.load_object(ObjectRef::new(4 + i as u32, 0)).unwrap();
            let mut out = Vec::new();
            serializer.write_indirect(&mut out, 4 + i as u32, 0, &object).unwrap();
            let want = plaintext(i);
            let at = out.windows(8).position(|w| w == b"\nstream\n").unwrap() + 8;
            assert_eq!(&out[at..at + want.len()], want.as_slice(), "{algorithm:?} stream {i}");
            let length = format!("/Length {}", want.len());
            assert!(out.windows(length.len()).any(|w| w == length.as_bytes()), "{algorithm:?} stream {i}");
            assert!(out.ends_with(b"\nendstream\nendobj\n"));
        }
    }
}

/// No Dart battery sees it: it decrypts an encrypted file and encrypts it again under every algorithm in one pass, then compares each stream with the plain source.
#[test]
fn an_encrypted_source_saved_under_every_algorithm_round_trips() {
    let first = encrypted_for(plain_source(), EncryptionAlgorithm::Aes256, "");
    for algorithm in ALGORITHMS {
        let mut source = DocumentEditor::from_bytes(first.clone()).unwrap();
        let config = EncryptionConfig::new("again", "owner").with_algorithm(algorithm);
        let second = source
            .save_to_bytes_with_options(SaveOptions {
                compress: false,
                ..SaveOptions::with_encryption(config)
            })
            .unwrap();
        let doc = PdfDocument::from_bytes(second).unwrap();
        assert!(doc.authenticate(b"again").unwrap(), "{algorithm:?}");
        for i in 0..LENGTHS.len() {
            let (_, extent) = extent_of(&doc, 4 + i as u32);
            assert_eq!(
                StreamBody::Extent(extent).read_whole_raw().unwrap(),
                plaintext(i),
                "{algorithm:?} stream {i}"
            );
        }
    }
}

/// No Dart battery sees it: a bare `decode_stream_data()` on an engine object is not a call Dart makes; the committed code returned the ciphertext here.
#[test]
fn a_bare_decode_returns_plaintext_for_an_encrypted_stream() {
    for algorithm in ALGORITHMS {
        let doc = PdfDocument::from_bytes(encrypted(plain_source(), algorithm)).unwrap();
        assert!(doc.authenticate(b"user").unwrap());
        let object = doc.load_object(ObjectRef::new(8, 0)).unwrap();
        assert_eq!(object.decode_stream_data().unwrap(), plaintext(4), "{algorithm:?}");
    }
}

/// No Dart battery sees it: RC4 is a keystream the engine owns; Dart only sees files it produced.
#[test]
fn the_rc4_keystream_in_pieces_equals_the_one_shot() {
    let sym = active().symmetric();
    let key = b"a key of some length";
    let data: Vec<u8> = (0..5000u32).map(|i| (i % 253) as u8).collect();
    let one_shot = sym.rc4(key, &data).unwrap();
    let mut stream = sym.rc4_stream(key).unwrap();
    let mut pieces = data.clone();
    for piece in pieces.chunks_mut(37) {
        stream.apply(piece);
    }
    assert_eq!(pieces, one_shot);
}

#[cfg(feature = "fips")]
/// No Dart battery sees it: the FIPS provider is a Rust build feature, and the refusal is of a primitive no Dart call names.
#[test]
fn the_fips_provider_refuses_rc4_stream() {
    use crate::crypto::CryptoProvider;
    let provider = crate::crypto::AwsLcProvider::new();
    let refused = provider.symmetric().rc4_stream(b"key");
    assert!(matches!(refused, Err(crate::crypto::Error::AlgorithmNotPermitted { .. })));
}
