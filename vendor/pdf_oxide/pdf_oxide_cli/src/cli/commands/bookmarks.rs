use pdf_oxide::outline::{Destination, OutlineItem};
use std::path::Path;

pub fn run(file: &Path, password: Option<&str>, json: bool) -> pdf_oxide::Result<()> {
    let doc = super::open_doc(file, password)?;

    let outline = doc.get_outline()?;

    match outline {
        None => {
            if json {
                super::write_output("[]", None)?;
            } else {
                eprintln!("No bookmarks found in {}", file.display());
            }
        },
        Some(items) => {
            if json {
                let json_items = outline_to_json(&items);
                let out = serde_json::to_string_pretty(&json_items).unwrap();
                super::write_output(&out, None)?;
            } else {
                for item in &items {
                    print_outline(item, 0);
                }
            }
        },
    }

    Ok(())
}

fn print_outline(item: &OutlineItem, depth: usize) {
    let indent = "  ".repeat(depth);
    let page = match &item.dest {
        Some(Destination::PageIndex(idx)) => format!(" (p. {})", idx + 1),
        Some(Destination::Named(name)) => format!(" (dest: {name})"),
        None => String::new(),
    };
    println!("{indent}{}{page}", item.title);
    for child in &item.children {
        print_outline(child, depth + 1);
    }
}

fn outline_to_json(items: &[OutlineItem]) -> Vec<serde_json::Value> {
    items
        .iter()
        .map(|item| {
            let page = match &item.dest {
                Some(Destination::PageIndex(idx)) => serde_json::json!(idx + 1),
                Some(Destination::Named(name)) => serde_json::json!(name),
                None => serde_json::Value::Null,
            };
            serde_json::json!({
                "title": item.title,
                "page": page,
                "children": outline_to_json(&item.children),
            })
        })
        .collect()
}
