//! A garbage-collecting save writes what the output names, not what the
//! source did.

use super::support::{contains, pdf_from_objects, stream_object};
use crate::editor::form_fields::FormFieldValue;
use crate::editor::{DocumentEditor, SaveOptions};

const MARK: &[u8] = b"ORPHANED APPEARANCE MARK";

/// A one-field form whose widget appearance carries `MARK`.
fn form_bytes() -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R] /DA (/Helv 0 Tf 0 g) /DR << /Font << /Helv 5 0 R >> >> >> >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /Helv 5 0 R >> >> /Contents 7 0 R /Annots [4 0 R] >>\nendobj\n".to_vec(),
        b"4 0 obj\n<< /FT /Tx /T (name) /Type /Annot /Subtype /Widget /Rect [72 700 400 720] /DA (/Helv 0 Tf 0 g) /AP << /N 6 0 R >> >>\nendobj\n".to_vec(),
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        stream_object(6, "/Type /XObject /Subtype /Form /BBox [0 0 328 20]", &[b"/Tx BMC\n% ".as_slice(), MARK, b"\nEMC\n"].concat()),
        stream_object(7, "", b"q Q\n"),
    ])
}

fn save(flatten: bool) -> Vec<u8> {
    let mut editor = DocumentEditor::from_bytes(form_bytes()).unwrap();
    editor.set_form_field_value("name", FormFieldValue::Text("Hello".into())).unwrap();
    if flatten {
        editor.flatten_forms().unwrap();
    }
    let options = SaveOptions { object_streams: false, ..SaveOptions::full_rewrite() };
    editor.save_to_bytes_with_options(options).unwrap()
}

/// No Dart battery sees it: Dart reads the document through its page tree, and the orphan hangs off no object a reader reaches; only the raw bytes of the file hold it.
#[test]
fn a_flattened_widget_appearance_the_output_no_longer_names_is_not_written() {
    assert!(contains(&save(false), MARK), "the unflattened save keeps the appearance");
    assert!(!contains(&save(true), MARK), "the flattened save wrote an orphaned appearance");
}

/// A page whose content (object 4) and a catalog `/Extra` stream (object 5) state their `/Length` as the integer objects 6 and 7.
fn indirect_length_pdf() -> Vec<u8> {
    let content = b"0 0 10 10 re f";
    let extra = b"extra stream data";
    super::support::pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Extra 5 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents 4 0 R >>\nendobj\n"
            .to_vec(),
        [
            b"4 0 obj\n<< /Length 6 0 R >>\nstream\n".as_slice(),
            content,
            b"\nendstream\nendobj\n",
        ]
        .concat(),
        [
            b"5 0 obj\n<< /Length 7 0 R >>\nstream\n".as_slice(),
            extra,
            b"\nendstream\nendobj\n",
        ]
        .concat(),
        format!("6 0 obj\n{}\nendobj\n", content.len()).into_bytes(),
        format!("7 0 obj\n{}\nendobj\n", extra.len()).into_bytes(),
    ])
}

/// No Dart battery sees it: Dart reads a saved file's objects, and a stray integer object nothing names is not part of any read.
#[test]
fn an_indirect_length_object_only_its_stream_names_is_not_written() {
    use crate::document::PdfDocument;
    use crate::editor::SaveOptions;
    use crate::object::{Object, ObjectRef};

    let mut editor = DocumentEditor::from_bytes(indirect_length_pdf()).unwrap();
    let options = SaveOptions { object_streams: false, ..SaveOptions::full_rewrite() };
    let bytes = editor.save_to_bytes_with_options(options).unwrap();
    let saved = PdfDocument::from_bytes(bytes).unwrap();

    for id in [6, 7] {
        let object = saved.load_object(ObjectRef::new(id, 0)).unwrap();
        assert!(matches!(object, Object::Null), "object {id} was written: {object:?}");
    }
    let content = saved.load_object(ObjectRef::new(4, 0)).unwrap();
    assert_eq!(content.decode_stream_data().unwrap(), b"0 0 10 10 re f");
    let extra = saved.load_object(ObjectRef::new(5, 0)).unwrap();
    assert_eq!(extra.decode_stream_data().unwrap(), b"extra stream data");
    let Object::Stream { dict, .. } = extra else { panic!("not a stream") };
    assert!(matches!(dict.get("Length"), Some(Object::Integer(17))), "{:?}", dict.get("Length"));
}
