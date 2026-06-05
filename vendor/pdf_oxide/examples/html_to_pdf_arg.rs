//! Render any HTML file to PDF via pdf_oxide::api::Pdf::from_html_css.
//! Used by the cross-rendering harness against WeasyPrint.

use pdf_oxide::api::Pdf;
use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: {} <input.html> <output.pdf>", args[0]);
        return Ok(ExitCode::from(1));
    }
    let input = &args[1];
    let output = &args[2];
    let html = fs::read_to_string(input)?;
    let font_path = Path::new("tests/fixtures/fonts/DejaVuSans.ttf");
    if !font_path.exists() {
        eprintln!("Run from the repo root so {} is reachable.", font_path.display());
        return Ok(ExitCode::from(2));
    }
    let font_bytes = fs::read(font_path)?;
    let mut pdf = Pdf::from_html_css(&html, "", font_bytes)?;
    pdf.save(output)?;
    eprintln!("wrote {output}");
    Ok(ExitCode::SUCCESS)
}
