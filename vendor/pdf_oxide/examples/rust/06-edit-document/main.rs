// Open a PDF, modify metadata, and save to a new file.
// Run: cargo run --example tutorial_edit_document -- tests/fixtures/simple.pdf /tmp/edited.pdf

use pdf_oxide::editor::DocumentEditor;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: tutorial_edit_document <input.pdf> <output.pdf>");
        std::process::exit(1);
    }
    let input = &args[1];
    let output = &args[2];

    let mut editor = DocumentEditor::open(input)?;
    println!("Opened: {}", input);

    editor.set_title("Edited Document");
    println!("Set title: \"Edited Document\"");

    editor.set_author("pdf_oxide");
    println!("Set author: \"pdf_oxide\"");

    let bytes = editor.save_to_bytes()?;
    std::fs::write(output, &bytes)?;
    println!("Saved: {}", output);

    Ok(())
}
