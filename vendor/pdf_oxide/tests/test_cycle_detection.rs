use pdf_oxide::document::PdfDocument;

fn write_temp_pdf(data: &[u8], name: &str) -> std::path::PathBuf {
    use std::io::Write;
    let dir = std::env::temp_dir().join("pdf_oxide_cycle_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(data).unwrap();
    path
}

fn finalize_pdf(pdf: &mut Vec<u8>, obj_offsets: &[usize]) {
    let xref_offset = pdf.len();
    let count = obj_offsets.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", count).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \r\n");
    for &off in &obj_offsets[1..] {
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", off).as_bytes());
    }
    let trailer = format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        count, xref_offset
    );
    pdf.extend_from_slice(trailer.as_bytes());
}

// --- XRef /Prev chain cycle detection ---

mod xref_prev_chain {
    use super::*;

    fn build_incremental_xref_chain(section_count: usize) -> Vec<u8> {
        assert!(section_count >= 1);

        let objects = b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj

2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj

3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>
endobj

";
        let mut pdf = objects.to_vec();
        let mut xref_offsets: Vec<usize> = Vec::new();

        for i in 0..section_count {
            let xref_start = pdf.len();
            xref_offsets.push(xref_start);

            pdf.extend_from_slice(b"xref\n");
            if i == 0 {
                pdf.extend_from_slice(b"0 4\n");
                pdf.extend_from_slice(b"0000000000 65535 f \r\n");
                pdf.extend_from_slice(format!("{:010} 00000 n \r\n", 9).as_bytes());
                pdf.extend_from_slice(format!("{:010} 00000 n \r\n", 58).as_bytes());
                pdf.extend_from_slice(format!("{:010} 00000 n \r\n", 115).as_bytes());
            } else {
                pdf.extend_from_slice(b"0 1\n");
                pdf.extend_from_slice(b"0000000000 65535 f \r\n");
            }

            pdf.extend_from_slice(b"trailer\n<< /Size 4 /Root 1 0 R");
            if i > 0 {
                pdf.extend_from_slice(format!(" /Prev {}", xref_offsets[i - 1]).as_bytes());
            }
            pdf.extend_from_slice(b" >>\n");
        }

        let last_xref = xref_offsets.last().unwrap();
        pdf.extend_from_slice(format!("startxref\n{}\n%%EOF\n", last_xref).as_bytes());
        pdf
    }

    fn build_self_referencing_xref() -> Vec<u8> {
        let objects = b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj

2 0 obj
<< /Type /Pages /Kids [] /Count 0 >>
endobj

";
        let mut pdf = objects.to_vec();
        let xref_start = pdf.len();

        pdf.extend_from_slice(b"xref\n0 3\n");
        pdf.extend_from_slice(b"0000000000 65535 f \r\n");
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", 9).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", 58).as_bytes());

        let trailer = format!("trailer\n<< /Size 3 /Root 1 0 R /Prev {} >>\n", xref_start);
        pdf.extend_from_slice(trailer.as_bytes());
        pdf.extend_from_slice(format!("startxref\n{}\n%%EOF\n", xref_start).as_bytes());
        pdf
    }

    fn build_two_node_xref_cycle() -> Vec<u8> {
        let objects = b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj

2 0 obj
<< /Type /Pages /Kids [] /Count 0 >>
endobj

";
        let mut pdf = objects.to_vec();

        let offset_a = pdf.len();
        pdf.extend_from_slice(b"xref\n0 3\n");
        pdf.extend_from_slice(b"0000000000 65535 f \r\n");
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", 9).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", 58).as_bytes());

        pdf.extend_from_slice(b"trailer\n<< /Size 3 /Root 1 0 R /Prev ");
        let prev_placeholder_pos = pdf.len();
        pdf.extend_from_slice(b"XXXXXXXXXX >>\n");

        let offset_b = pdf.len();
        pdf.extend_from_slice(b"xref\n0 1\n");
        pdf.extend_from_slice(b"0000000000 65535 f \r\n");
        let trailer_b = format!("trailer\n<< /Size 3 /Root 1 0 R /Prev {} >>\n", offset_a);
        pdf.extend_from_slice(trailer_b.as_bytes());

        let offset_b_str = format!("{:<10}", offset_b);
        pdf[prev_placeholder_pos..prev_placeholder_pos + 10]
            .copy_from_slice(offset_b_str.as_bytes());

        pdf.extend_from_slice(format!("startxref\n{}\n%%EOF\n", offset_b).as_bytes());
        pdf
    }

    #[test]
    fn three_section_chain_opens_successfully() {
        let data = build_incremental_xref_chain(3);
        let path = write_temp_pdf(&data, "xref_3_sections.pdf");
        let result = PdfDocument::open(&path);
        assert!(result.is_ok(), "3 incremental xref sections should open: {:?}", result.err());
    }

    #[test]
    fn five_section_deep_chain_opens_successfully() {
        let data = build_incremental_xref_chain(5);
        let path = write_temp_pdf(&data, "xref_5_sections.pdf");
        let result = PdfDocument::open(&path);
        assert!(result.is_ok(), "5 incremental xref sections should open: {:?}", result.err());
    }

    #[test]
    fn self_referencing_prev_terminates_without_hang() {
        let data = build_self_referencing_xref();
        let path = write_temp_pdf(&data, "xref_self_loop.pdf");
        let _result = PdfDocument::open(&path);
    }

    #[test]
    fn two_node_prev_cycle_terminates_without_hang() {
        let data = build_two_node_xref_cycle();
        let path = write_temp_pdf(&data, "xref_two_node_cycle.pdf");
        let _result = PdfDocument::open(&path);
    }
}

// --- Circular Form XObject references ---

mod circular_xobject {
    use super::*;

    fn build_self_referencing_form_xobject() -> Vec<u8> {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");

        let obj1 = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\n");

        let obj2 = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\n");

        let obj3 = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
              /Resources << /XObject << /X0 4 0 R >> >> /Contents 5 0 R >>\nendobj\n\n",
        );

        let obj4 = pdf.len();
        let stream = b"/X0 Do";
        let header = format!(
            "4 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /XObject << /X0 4 0 R >> >> /Length {} >>\nstream\n",
            stream.len()
        );
        pdf.extend_from_slice(header.as_bytes());
        pdf.extend_from_slice(stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

        let obj5 = pdf.len();
        let content = b"/X0 Do";
        let content_header = format!("5 0 obj\n<< /Length {} >>\nstream\n", content.len());
        pdf.extend_from_slice(content_header.as_bytes());
        pdf.extend_from_slice(content);
        pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

        finalize_pdf(&mut pdf, &[0, obj1, obj2, obj3, obj4, obj5]);
        pdf
    }

    fn build_two_node_xobject_cycle() -> Vec<u8> {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");

        let obj1 = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\n");

        let obj2 = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\n");

        let obj3 = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
              /Resources << /XObject << /X0 4 0 R /X1 5 0 R >> >> /Contents 6 0 R >>\nendobj\n\n",
        );

        let obj4 = pdf.len();
        let stream = b"/X1 Do";
        let header = format!(
            "4 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /XObject << /X1 5 0 R >> >> /Length {} >>\nstream\n",
            stream.len()
        );
        pdf.extend_from_slice(header.as_bytes());
        pdf.extend_from_slice(stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

        let obj5 = pdf.len();
        let stream = b"/X0 Do";
        let header = format!(
            "5 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /XObject << /X0 4 0 R >> >> /Length {} >>\nstream\n",
            stream.len()
        );
        pdf.extend_from_slice(header.as_bytes());
        pdf.extend_from_slice(stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

        let obj6 = pdf.len();
        let content = b"/X0 Do";
        let header = format!("6 0 obj\n<< /Length {} >>\nstream\n", content.len());
        pdf.extend_from_slice(header.as_bytes());
        pdf.extend_from_slice(content);
        pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

        finalize_pdf(&mut pdf, &[0, obj1, obj2, obj3, obj4, obj5, obj6]);
        pdf
    }

    fn build_three_node_xobject_cycle() -> Vec<u8> {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");

        let obj1 = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\n");

        let obj2 = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\n");

        let obj3 = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
              /Resources << /XObject << /X0 4 0 R /X1 5 0 R /X2 6 0 R >> >> \
              /Contents 7 0 R >>\nendobj\n\n",
        );

        let obj4 = pdf.len();
        let stream = b"/X1 Do";
        let header = format!(
            "4 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /XObject << /X1 5 0 R >> >> /Length {} >>\nstream\n",
            stream.len()
        );
        pdf.extend_from_slice(header.as_bytes());
        pdf.extend_from_slice(stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

        let obj5 = pdf.len();
        let stream = b"/X2 Do";
        let header = format!(
            "5 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /XObject << /X2 6 0 R >> >> /Length {} >>\nstream\n",
            stream.len()
        );
        pdf.extend_from_slice(header.as_bytes());
        pdf.extend_from_slice(stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

        let obj6 = pdf.len();
        let stream = b"/X0 Do";
        let header = format!(
            "6 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /XObject << /X0 4 0 R >> >> /Length {} >>\nstream\n",
            stream.len()
        );
        pdf.extend_from_slice(header.as_bytes());
        pdf.extend_from_slice(stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

        let obj7 = pdf.len();
        let content = b"/X0 Do";
        let header = format!("7 0 obj\n<< /Length {} >>\nstream\n", content.len());
        pdf.extend_from_slice(header.as_bytes());
        pdf.extend_from_slice(content);
        pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

        finalize_pdf(&mut pdf, &[0, obj1, obj2, obj3, obj4, obj5, obj6, obj7]);
        pdf
    }

    fn build_reused_form_xobject() -> Vec<u8> {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");

        let obj1 = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\n");

        let obj2 = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\n");

        let obj3 = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
              /Resources << /Font << /F1 5 0 R >> /XObject << /X0 4 0 R >> >> \
              /Contents 6 0 R >>\nendobj\n\n",
        );

        let obj4 = pdf.len();
        let stream = b"BT /F1 12 Tf 10 10 Td (Reused) Tj ET";
        let header = format!(
            "4 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /Font << /F1 5 0 R >> >> /Length {} >>\nstream\n",
            stream.len()
        );
        pdf.extend_from_slice(header.as_bytes());
        pdf.extend_from_slice(stream);
        pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

        let obj5 = pdf.len();
        pdf.extend_from_slice(
            b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica \
              /Encoding /WinAnsiEncoding >>\nendobj\n\n",
        );

        let obj6 = pdf.len();
        let content = b"q /X0 Do Q q /X0 Do Q";
        let header = format!("6 0 obj\n<< /Length {} >>\nstream\n", content.len());
        pdf.extend_from_slice(header.as_bytes());
        pdf.extend_from_slice(content);
        pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

        finalize_pdf(&mut pdf, &[0, obj1, obj2, obj3, obj4, obj5, obj6]);
        pdf
    }

    #[test]
    fn self_referencing_xobject_terminates_without_overflow() {
        let data = build_self_referencing_form_xobject();
        let path = write_temp_pdf(&data, "xobj_self_ref.pdf");
        let doc = PdfDocument::open(&path).expect("Should parse PDF structure");
        let _result = doc.extract_text(0);
    }

    #[test]
    fn two_node_xobject_cycle_terminates_without_overflow() {
        let data = build_two_node_xobject_cycle();
        let path = write_temp_pdf(&data, "xobj_two_node_cycle.pdf");
        let doc = PdfDocument::open(&path).expect("Should parse PDF structure");
        let _result = doc.extract_text(0);
    }

    #[test]
    fn three_node_xobject_cycle_terminates_gracefully() {
        let data = build_three_node_xobject_cycle();
        let path = write_temp_pdf(&data, "xobj_three_node_cycle.pdf");
        let doc = PdfDocument::open(&path).expect("Should parse PDF structure");
        let _result = doc.extract_text(0);
    }

    #[test]
    fn non_circular_xobject_invoked_twice_produces_text() {
        let data = build_reused_form_xobject();
        let path = write_temp_pdf(&data, "xobj_reused_twice.pdf");
        let doc = PdfDocument::open(&path).expect("Should parse PDF structure");
        let text = doc.extract_text(0).unwrap();
        assert!(
            text.contains("Reused"),
            "Reused XObject text should appear at least once: got '{}'",
            text
        );
    }
}

// --- Cyclic structure tree /K references (SIGSEGV regression) ---

mod struct_tree_cycle {
    use super::*;

    /// Build a tagged PDF whose structure tree has a two-node cycle:
    /// StructElem A (obj 5) /K → B (obj 6), StructElem B (obj 6) /K → A (obj 5).
    fn build_two_node_struct_cycle() -> Vec<u8> {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.7\n");

        let obj1 = pdf.len();
        pdf.extend_from_slice(
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /StructTreeRoot 4 0 R >>\nendobj\n\n",
        );

        let obj2 = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\n");

        let obj3 = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\n",
        );

        // StructTreeRoot with /K pointing to element A
        let obj4 = pdf.len();
        pdf.extend_from_slice(b"4 0 obj\n<< /Type /StructTreeRoot /K [5 0 R] >>\nendobj\n\n");

        // StructElem A: /K → B (6 0 R)
        let obj5 = pdf.len();
        pdf.extend_from_slice(b"5 0 obj\n<< /Type /StructElem /S /P /K 6 0 R >>\nendobj\n\n");

        // StructElem B: /K → A (5 0 R) — creates the cycle
        let obj6 = pdf.len();
        pdf.extend_from_slice(b"6 0 obj\n<< /Type /StructElem /S /P /K 5 0 R >>\nendobj\n\n");

        finalize_pdf(&mut pdf, &[0, obj1, obj2, obj3, obj4, obj5, obj6]);
        pdf
    }

    /// Build a tagged PDF whose structure tree has a self-referencing element:
    /// StructElem A (obj 5) /K → A (5 0 R).
    fn build_self_referencing_struct_elem() -> Vec<u8> {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.7\n");

        let obj1 = pdf.len();
        pdf.extend_from_slice(
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /StructTreeRoot 4 0 R >>\nendobj\n\n",
        );

        let obj2 = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\n");

        let obj3 = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\n",
        );

        let obj4 = pdf.len();
        pdf.extend_from_slice(b"4 0 obj\n<< /Type /StructTreeRoot /K [5 0 R] >>\nendobj\n\n");

        // StructElem A: /K → itself (5 0 R)
        let obj5 = pdf.len();
        pdf.extend_from_slice(b"5 0 obj\n<< /Type /StructElem /S /P /K 5 0 R >>\nendobj\n\n");

        finalize_pdf(&mut pdf, &[0, obj1, obj2, obj3, obj4, obj5]);
        pdf
    }

    /// Build a tagged PDF with a three-node structure tree cycle:
    /// A → B → C → A.
    fn build_three_node_struct_cycle() -> Vec<u8> {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.7\n");

        let obj1 = pdf.len();
        pdf.extend_from_slice(
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /StructTreeRoot 4 0 R >>\nendobj\n\n",
        );

        let obj2 = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\n");

        let obj3 = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\n",
        );

        let obj4 = pdf.len();
        pdf.extend_from_slice(b"4 0 obj\n<< /Type /StructTreeRoot /K [5 0 R] >>\nendobj\n\n");

        // A → B
        let obj5 = pdf.len();
        pdf.extend_from_slice(b"5 0 obj\n<< /Type /StructElem /S /P /K 6 0 R >>\nendobj\n\n");

        // B → C
        let obj6 = pdf.len();
        pdf.extend_from_slice(b"6 0 obj\n<< /Type /StructElem /S /P /K 7 0 R >>\nendobj\n\n");

        // C → A (closes the cycle)
        let obj7 = pdf.len();
        pdf.extend_from_slice(b"7 0 obj\n<< /Type /StructElem /S /P /K 5 0 R >>\nendobj\n\n");

        finalize_pdf(&mut pdf, &[0, obj1, obj2, obj3, obj4, obj5, obj6, obj7]);
        pdf
    }

    #[test]
    fn two_node_struct_cycle_terminates_without_overflow() {
        let data = build_two_node_struct_cycle();
        let path = write_temp_pdf(&data, "struct_two_node_cycle.pdf");
        let doc = PdfDocument::open(&path).expect("Should parse PDF structure");
        // Must not SIGSEGV or hang — the visited-set guard must break the cycle.
        let _result = doc.extract_text(0);
    }

    #[test]
    fn self_referencing_struct_elem_terminates_without_overflow() {
        let data = build_self_referencing_struct_elem();
        let path = write_temp_pdf(&data, "struct_self_ref.pdf");
        let doc = PdfDocument::open(&path).expect("Should parse PDF structure");
        let _result = doc.extract_text(0);
    }

    #[test]
    fn three_node_struct_cycle_terminates_without_overflow() {
        let data = build_three_node_struct_cycle();
        let path = write_temp_pdf(&data, "struct_three_node_cycle.pdf");
        let doc = PdfDocument::open(&path).expect("Should parse PDF structure");
        let _result = doc.extract_text(0);
    }
}
