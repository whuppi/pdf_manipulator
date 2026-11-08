//! Edits that name a set of pages and the metadata replacement: each is
//! one engine call that checks everything before it changes anything.

use super::support::{open_editor_from_bytes, pdf_from_objects};
use crate::cancel::CancelCheck;
use crate::editor::{DocumentInfo, SaveOptions};
use crate::host::dispatch;

/// Three pages; the first has its own crop box.
fn three_pages() -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200] /CropBox [10 10 90 190] >>\nendobj\n".to_vec(),
        b"4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200] >>\nendobj\n".to_vec(),
        b"5 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200] >>\nendobj\n".to_vec(),
    ])
}

fn rotations(editor: &mut crate::editor::DocumentEditor) -> Vec<i32> {
    (0..editor.current_page_count())
        .map(|i| editor.get_page_rotation(i).unwrap())
        .collect()
}

/// A Dart battery sees only that the call failed: this reads the editor's
/// pages afterwards to prove no page was turned before the bad one was found.
#[test]
fn a_bad_page_in_the_list_leaves_every_page_unturned() {
    let mut editor = open_editor_from_bytes(&three_pages(), "bad_page_rotate");
    assert!(dispatch::edit_rotate_pages(&mut editor, &[0, 1, 7], 90).is_err());
    assert_eq!(rotations(&mut editor), [0, 0, 0]);
    assert!(dispatch::edit_set_rotation(&mut editor, &[2, 9], 180).is_err());
    assert_eq!(rotations(&mut editor), [0, 0, 0]);
    assert!(!editor.is_modified());
}

/// A repeated page would turn twice or be deleted twice; the engine refuses
/// it, and an empty list, before it touches a page.
#[test]
fn repeated_and_empty_page_lists_are_refused() {
    let mut editor = open_editor_from_bytes(&three_pages(), "repeated_pages");
    assert!(dispatch::edit_rotate_pages(&mut editor, &[1, 1], 90).is_err());
    assert!(dispatch::edit_rotate_pages(&mut editor, &[], 90).is_err());
    assert!(dispatch::edit_delete_pages(&mut editor, &[0, 0]).is_err());
    assert!(dispatch::edit_set_media_box(&mut editor, &[], [0.0, 0.0, 1.0, 1.0]).is_err());
    assert_eq!(rotations(&mut editor), [0, 0, 0]);
    assert_eq!(editor.current_page_count(), 3);
}

/// Deleting the last page of a document is an error that leaves the
/// document as it was.
#[test]
fn deleting_every_page_is_refused_and_changes_nothing() {
    let mut editor = open_editor_from_bytes(&three_pages(), "delete_all");
    assert!(dispatch::edit_delete_pages(&mut editor, &[0, 1, 2]).is_err());
    assert_eq!(editor.current_page_count(), 3);
    dispatch::edit_delete_pages(&mut editor, &[0, 2]).unwrap();
    assert_eq!(editor.current_page_count(), 1);
}

/// Relative and absolute rotation over one list, read back through the
/// editor's page facts, which the Dart `pageInfo` of an editor returns.
#[test]
fn rotate_and_set_rotation_cover_every_page_named() {
    let mut editor = open_editor_from_bytes(&three_pages(), "rotate_set");
    dispatch::edit_rotate_pages(&mut editor, &[0, 2], 90).unwrap();
    dispatch::edit_rotate_pages(&mut editor, &[0], 90).unwrap();
    assert_eq!(rotations(&mut editor), [180, 0, 90]);
    dispatch::edit_set_rotation(&mut editor, &[0, 1], 270).unwrap();
    let infos = dispatch::edit_page_info(&mut editor, &[0, 1, 2], &CancelCheck::none()).unwrap();
    let turned: Vec<i32> = infos.iter().map(|p| p.rotation).collect();
    assert_eq!(turned, [270, 270, 90]);
}

/// Dart reads a page's own crop box as absent; removal must survive a save
/// and reopen, which only the engine's page-props path can show.
#[test]
fn removing_a_crop_box_survives_save_and_reopen() {
    let mut editor = open_editor_from_bytes(&three_pages(), "crop_remove");
    let before = dispatch::edit_page_info(&mut editor, &[0], &CancelCheck::none()).unwrap();
    assert_eq!(before[0].crop_box, Some([10.0, 10.0, 80.0, 180.0]));

    dispatch::edit_set_crop_box(&mut editor, &[0, 1], None).unwrap();
    let after = dispatch::edit_page_info(&mut editor, &[0, 1], &CancelCheck::none()).unwrap();
    assert!(after.iter().all(|p| p.crop_box.is_none()));

    let saved = editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();
    let mut reopened = open_editor_from_bytes(&saved, "crop_remove_reopened");
    assert_eq!(reopened.get_page_crop_box(0).unwrap(), None);

    dispatch::edit_set_crop_box(&mut reopened, &[1], Some([5.0, 5.0, 50.0, 60.0])).unwrap();
    assert_eq!(reopened.get_page_crop_box(1).unwrap(), Some([5.0, 5.0, 50.0, 60.0]));
}

/// The media box of every page named changes together, and the page facts
/// report the origin and size the wire carries.
#[test]
fn set_media_box_covers_every_page_named() {
    let mut editor = open_editor_from_bytes(&three_pages(), "media_set");
    dispatch::edit_set_media_box(&mut editor, &[1, 2], [5.0, 15.0, 405.0, 515.0]).unwrap();
    let infos = dispatch::edit_page_info(&mut editor, &[0, 1, 2], &CancelCheck::none()).unwrap();
    assert_eq!((infos[0].x, infos[0].width), (0.0, 100.0));
    assert_eq!((infos[1].x, infos[1].y, infos[1].width, infos[1].height), (5.0, 15.0, 400.0, 500.0));
    assert_eq!((infos[2].x, infos[2].width), (5.0, 400.0));
}

/// A field the new metadata leaves out is gone from the saved file, which
/// no Dart battery can see without reading the raw info dictionary.
#[test]
fn set_metadata_removes_the_fields_it_leaves_out() {
    let mut editor = open_editor_from_bytes(&three_pages(), "metadata_replace");
    dispatch::edit_set_metadata(
        &mut editor,
        DocumentInfo {
            title: Some("T".into()),
            author: Some("A".into()),
            ..Default::default()
        },
    )
    .unwrap();
    dispatch::edit_set_metadata(
        &mut editor,
        DocumentInfo { title: Some("Only title".into()), ..Default::default() },
    )
    .unwrap();
    let saved = editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();
    let mut reopened = open_editor_from_bytes(&saved, "metadata_replace_reopened");
    let m = dispatch::edit_get_metadata(&mut reopened);
    assert_eq!(m.title, "Only title");
    assert_eq!(m.author, "");
}

/// A watermark that cannot be drawn on a later page changes nothing on the
/// earlier ones: a Dart battery sees the error, not the bytes the editor
/// would have saved. The last page is merged in, and a merged page takes
/// neither an annotation nor a font of its own.
#[test]
fn a_watermark_that_fails_on_a_later_page_changes_nothing() {
    for layer in ["foreground", "background"] {
        for listed in [Some([0usize, 3]), None] {
            let mut editor = open_editor_from_bytes(&three_pages(), "watermark_later_fails");
            editor.merge_from_bytes(&three_pages()).unwrap();
            let before = editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();
            let result = dispatch::edit_watermark(
                &mut editor,
                listed.as_ref().map(|p| &p[..]),
                "W", 12.0, 0.0, 1.0, 0.0, 0.0, 0.0, layer, dispatch::WatermarkPosition::Centre,
            );
            assert!(result.is_err(), "{layer} {listed:?}");
            let after = editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();
            assert!(before == after, "{layer} {listed:?}: the failed call changed the saved bytes");
        }
    }
}

/// A watermark over a list that names a missing page refuses before any
/// page is stamped.
#[test]
fn a_watermark_over_a_bad_page_list_stamps_nothing() {
    let mut editor = open_editor_from_bytes(&three_pages(), "watermark_bad");
    let result = dispatch::edit_watermark(
        &mut editor, Some(&[0, 9]), "W", 12.0, 0.0, 1.0, 0.0, 0.0, 0.0, "foreground", dispatch::WatermarkPosition::Centre,
    );
    assert!(result.is_err());
    assert!(!editor.is_modified());
}
