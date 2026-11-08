//! The checks behind a save that keeps the source's encryption.

use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, EncryptionConfig, SaveOptions};
use crate::encryption::{Algorithm, EncryptDictBuilder};
use crate::error::Error;
use crate::writer::ObjectSerializer;

const FILE_ID: [u8; 16] = [0x11; 16];

/// An AES-256 source whose /Encrypt dictionary is the engine writer's own
/// (`EncryptDictBuilder`), since wrapping the file key by hand is the
/// engine's job. Every offset is computed here.
fn source(user_password: &[u8]) -> Vec<u8> {
    let (dict, _key) = EncryptDictBuilder::new(Algorithm::Aes256)
        .user_password(user_password)
        .owner_password(b"owner")
        .permissions(-3900)
        .encrypt_metadata(true)
        .build_with_key(&FILE_ID)
        .unwrap();
    let encrypt = dict.to_object();
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n".to_vec(),
        ObjectSerializer::compact().serialize_indirect(4, 0, &encrypt),
    ];
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for o in &objects {
        offsets.push(pdf.len());
        pdf.extend_from_slice(o);
    }
    let xref_at = pdf.len();
    pdf.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
    for off in offsets {
        pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size 5 /Root 1 0 R /Encrypt 4 0 R {ID} >>\n\
             startxref\n{xref_at}\n%%EOF\n"
        )
        .as_bytes(),
    );
    pdf
}

const ID: &str = "/ID [<11111111111111111111111111111111> <22222222222222222222222222222222>]";

/// No Dart battery sees it: the state it needs, a source with a user password that the engine holds unauthenticated, cannot be reached from Dart, because `edit` throws `PdfPasswordRequired` on open and no save of such a source ever runs.
#[test]
fn refuses_an_unauthenticated_source() {
    let doc = PdfDocument::from_bytes(source(b"secret")).unwrap();
    match doc.kept_encryption() {
        Err(Error::Unsupported(msg)) => {
            assert!(
                msg.starts_with("keeping this source's encryption is not supported: ")
                    && msg.contains("the source is not authenticated")
                    && msg.ends_with("save it with explicit encryption"),
                "unexpected message: {msg}"
            );
        },
        Err(other) => panic!("expected Unsupported, got {other:?}"),
        Ok(_) => panic!("expected Unsupported, got Ok"),
    }
}

/// No Dart battery sees it: no Dart call builds save options that hold both a kept encryption and an explicit config, because `PdfEncryption` is a sealed choice of keep, remove or config and the host sets `keep_source_encryption` for keep alone.
#[test]
fn keep_and_an_explicit_config_exclude_each_other() {
    let doc = PdfDocument::from_bytes(source(b"")).unwrap();
    let mut editor = DocumentEditor::from_document(doc).unwrap();
    let options = SaveOptions {
        keep_source_encryption: true,
        encryption: Some(EncryptionConfig::new("user", "owner")),
        ..Default::default()
    };
    match editor.save_to_bytes_with_options(options) {
        Err(Error::InvalidOperation(msg)) => assert!(msg.contains("exclusive"), "{msg}"),
        Err(other) => panic!("expected InvalidOperation, got {other:?}"),
        Ok(_) => panic!("expected InvalidOperation, got Ok"),
    }
}
