/// Parse page range strings like "1-5", "1,3,7", "1-3,7,10-12" into 0-indexed page numbers.
pub fn parse_page_ranges(input: &str) -> Result<Vec<usize>, String> {
    let mut pages = Vec::new();

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some((start, end)) = part.split_once('-') {
            let start: usize = start
                .trim()
                .parse()
                .map_err(|_| format!("Invalid page number: '{}'", start.trim()))?;
            let end: usize = end
                .trim()
                .parse()
                .map_err(|_| format!("Invalid page number: '{}'", end.trim()))?;

            if start == 0 || end == 0 {
                return Err("Page numbers start at 1".to_string());
            }
            if start > end {
                return Err(format!("Invalid range: {start}-{end} (start > end)"));
            }

            for p in start..=end {
                pages.push(p - 1); // Convert to 0-indexed
            }
        } else {
            let page: usize = part
                .parse()
                .map_err(|_| format!("Invalid page number: '{part}'"))?;
            if page == 0 {
                return Err("Page numbers start at 1".to_string());
            }
            pages.push(page - 1); // Convert to 0-indexed
        }
    }

    if pages.is_empty() {
        return Err("No page numbers specified".to_string());
    }

    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_page() {
        assert_eq!(parse_page_ranges("1").unwrap(), vec![0]);
        assert_eq!(parse_page_ranges("5").unwrap(), vec![4]);
    }

    #[test]
    fn test_range() {
        assert_eq!(parse_page_ranges("1-3").unwrap(), vec![0, 1, 2]);
    }

    #[test]
    fn test_comma_separated() {
        assert_eq!(parse_page_ranges("1,3,7").unwrap(), vec![0, 2, 6]);
    }

    #[test]
    fn test_mixed() {
        assert_eq!(parse_page_ranges("1-3,7,10-12").unwrap(), vec![0, 1, 2, 6, 9, 10, 11]);
    }

    #[test]
    fn test_zero_rejected() {
        assert!(parse_page_ranges("0").is_err());
    }

    #[test]
    fn test_invalid_range() {
        assert!(parse_page_ranges("5-3").is_err());
    }
}
