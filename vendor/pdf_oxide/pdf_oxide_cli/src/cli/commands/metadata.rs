use pdf_oxide::editor::{DocumentEditor, DocumentInfo, EditableDocument, SaveOptions};
use std::path::Path;

#[allow(clippy::too_many_arguments)]
pub fn run(
    file: &Path,
    title: Option<&str>,
    author: Option<&str>,
    subject: Option<&str>,
    keywords: Option<&str>,
    strip: bool,
    output: Option<&Path>,
    _password: Option<&str>,
    json: bool,
) -> pdf_oxide::Result<()> {
    let is_write =
        strip || title.is_some() || author.is_some() || subject.is_some() || keywords.is_some();

    let mut editor = DocumentEditor::open(file)?;

    if !is_write {
        // Read-only: print current metadata
        let info = editor.get_info()?;

        if json {
            let json_out = serde_json::json!({
                "file": file.display().to_string(),
                "title": info.title,
                "author": info.author,
                "subject": info.subject,
                "keywords": info.keywords,
                "creator": info.creator,
                "producer": info.producer,
                "creation_date": info.creation_date,
                "mod_date": info.mod_date,
            });
            super::write_output(&serde_json::to_string_pretty(&json_out).unwrap(), None)?;
        } else {
            println!("File:          {}", file.display());
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
        return Ok(());
    }

    // Write mode
    if strip {
        editor.set_info(DocumentInfo::new())?;
        eprintln!("Stripped all metadata");
    } else {
        if let Some(t) = title {
            editor.set_title(t);
        }
        if let Some(a) = author {
            editor.set_author(a);
        }
        if let Some(s) = subject {
            editor.set_subject(s);
        }
        if let Some(k) = keywords {
            editor.set_keywords(k);
        }
    }

    let out_path = output.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        Path::new(&format!("{stem}_metadata.pdf")).to_path_buf()
    });

    editor.save_with_options(
        &out_path,
        SaveOptions {
            compress: true,
            garbage_collect: true,
            ..Default::default()
        },
    )?;

    eprintln!("Saved to {}", out_path.display());

    Ok(())
}
