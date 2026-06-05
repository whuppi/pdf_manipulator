use pdf_oxide::editor::{DocumentEditor, EditableDocument};
use std::path::Path;

pub fn run(file: &Path, password: Option<&str>, json: bool) -> pdf_oxide::Result<()> {
    let doc = super::open_doc(file, password)?;
    let page_count = doc.page_count()?;
    let (major, minor) = doc.version();

    let mut editor = DocumentEditor::open(file)?;
    let info = editor.get_info()?;

    if json {
        let json_out = serde_json::json!({
            "file": file.display().to_string(),
            "pages": page_count,
            "version": format!("{major}.{minor}"),
            "title": info.title,
            "author": info.author,
            "subject": info.subject,
            "keywords": info.keywords,
            "creator": info.creator,
            "producer": info.producer,
            "creation_date": info.creation_date,
            "mod_date": info.mod_date,
        });
        let out = serde_json::to_string_pretty(&json_out).unwrap();
        super::write_output(&out, None)?;
    } else {
        println!("File:          {}", file.display());
        println!("PDF version:   {major}.{minor}");
        println!("Pages:         {page_count}");
        if let Some(t) = &info.title {
            println!("Title:         {t}");
        }
        if let Some(a) = &info.author {
            println!("Author:        {a}");
        }
        if let Some(s) = &info.subject {
            println!("Subject:       {s}");
        }
        if let Some(k) = &info.keywords {
            println!("Keywords:      {k}");
        }
        if let Some(c) = &info.creator {
            println!("Creator:       {c}");
        }
        if let Some(p) = &info.producer {
            println!("Producer:      {p}");
        }
        if let Some(d) = &info.creation_date {
            println!("Created:       {d}");
        }
        if let Some(d) = &info.mod_date {
            println!("Modified:      {d}");
        }
    }

    Ok(())
}
