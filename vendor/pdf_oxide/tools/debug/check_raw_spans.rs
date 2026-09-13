use pdf_oxide::PdfDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pdf_path = "test_datasets/pdfs/academic/arxiv_2510.21165v1.pdf";

    println!("Opening: {}", pdf_path);
    let mut doc = PdfDocument::open(pdf_path)?;

    // Extract raw spans
    let spans = doc.extract_spans(0)?;

    println!("\n=== RAW SPANS (first 50) ===\n");

    for (i, span) in spans.iter().take(50).enumerate() {
        println!("{:3}. \"{}\" (font_size: {:.1}, bold: {})",
            i,
            span.text,
            span.font_size,
            span.font_weight.is_bold()
        );
    }

    // Check for word-splitting patterns in RAW spans
    println!("\n=== CHECKING FOR WORD-SPLITTING IN RAW SPANS ===\n");

    let patterns = ["var ious", "cor relation", "retur ns", "distr ibutions"];
    let mut found_in_spans = Vec::new();

    for span in &spans {
        for pattern in &patterns {
            if span.text.contains(pattern) {
                found_in_spans.push((pattern, span.text.clone()));
            }
        }
    }

    if found_in_spans.is_empty() {
        println!("✅ NO WORD-SPLITTING in raw spans!");
        println!("The issue is introduced AFTER span extraction.");
    } else {
        println!("❌ Word-splitting found in raw spans:");
        for (pattern, text) in found_in_spans {
            println!("  Pattern '{}' in span: \"{}\"", pattern, text);
        }
    }

    // Check individual span texts
    println!("\n=== ALL SPANS WITH 'var', 'cor', 'ret', 'dis' ===\n");

    for (i, span) in spans.iter().enumerate() {
        let lower = span.text.to_lowercase();
        if lower.contains("var") || lower.contains("cor") || lower.contains("ret") || lower.contains("dis") {
            println!("{:4}. \"{}\" at x={:.1}, y={:.1}", i, span.text, span.bbox.x, span.bbox.y);
        }
    }

    // Look for three-span patterns: "var" + " " + "ious"
    println!("\n=== LOOKING FOR THREE-SPAN PATTERNS (word + space + fragment) ===\n");
    for i in 0..spans.len()-2 {
        let text1 = &spans[i].text;
        let text2 = &spans[i+1].text;
        let text3 = &spans[i+2].text;
        let combined = format!("{}{}{}", text1, text2, text3);

        if (text1 == "var" || text1.starts_with("cor") || text1.starts_with("retur")) && text2 == " " {
            println!("Spans {}-{}: \"{}\" + \"{}\" + \"{}\" = \"{}\"",
                i, i+2, text1, text2, text3, combined);
        }
    }

    Ok(())
}
