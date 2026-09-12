// Hand-authored DOCX fixtures — the office-conversion counterpart of
// handwritten.dart. Built byte-by-byte here (independent producer; never
// this package's own convertTo writer, which would feed the engine its
// own output and mask mirrored reader/writer bugs).

import 'dart:typed_data';

import 'package:archive/archive.dart';

/// Marker tokens placed one per table column, searchable in the
/// converted PDF: `COLMARK01` … `COLMARK11`.
List<String> get docxWideTableMarkers =>
    List.generate(11, (i) => 'COLMARK${(i + 1).toString().padLeft(2, '0')}');

/// The exact declared column widths from issue #243, in twips.
/// Sum = 14,796 twips (10.275 in) on a landscape page whose usable
/// width (pgSz − margins) is 14,400 twips (10 in) — 2.6% over, which a
/// correct renderer honors nearly as-is or clips slightly.
const docxWideTableGridCols = [
  666, 1330, 1237, 1401, 1258, 1236, 2160, 1440, 1260, 1440, 1368, //
];

/// A minimal single-table DOCX reproducing issue #243: landscape
/// 15840x12240 twips, `tblLayout fixed`, `tblW auto 0`, eleven
/// `gridCol` widths as declared above, one row of ordinary prose cells.
/// The engine rendered this table ~3.4x its declared width, pushing
/// most columns past the 792 pt page edge.
Uint8List buildWideTableDocx() {
  final cells = StringBuffer();
  for (var i = 0; i < docxWideTableGridCols.length; i++) {
    cells.write('<w:tc><w:tcPr>'
        '<w:tcW w:w="${docxWideTableGridCols[i]}" w:type="dxa"/>'
        '</w:tcPr><w:p><w:r><w:t xml:space="preserve">'
        '${docxWideTableMarkers[i]} plain body prose that wraps fine'
        '</w:t></w:r></w:p></w:tc>');
  }
  final grid = docxWideTableGridCols
      .map((w) => '<w:gridCol w:w="$w"/>')
      .join();

  const ns = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';
  final document = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
      '<w:document xmlns:w="$ns"><w:body>'
      '<w:tbl><w:tblPr>'
      '<w:tblW w:w="0" w:type="auto"/>'
      '<w:tblLayout w:type="fixed"/>'
      '</w:tblPr>'
      '<w:tblGrid>$grid</w:tblGrid>'
      '<w:tr>$cells</w:tr>'
      '</w:tbl>'
      '<w:p/>'
      '<w:sectPr>'
      '<w:pgSz w:w="15840" w:h="12240" w:orient="landscape"/>'
      '<w:pgMar w:top="720" w:right="630" w:bottom="810" w:left="810" '
      'w:header="720" w:footer="720" w:gutter="0"/>'
      '</w:sectPr>'
      '</w:body></w:document>';

  const contentTypes =
      '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
      '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
      '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
      '<Default Extension="xml" ContentType="application/xml"/>'
      '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
      '</Types>';

  const rels = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
      '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
      '<Relationship Id="rId1" '
      'Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" '
      'Target="word/document.xml"/>'
      '</Relationships>';

  final archive = Archive()
    ..add(ArchiveFile.string('[Content_Types].xml', contentTypes))
    ..add(ArchiveFile.string('_rels/.rels', rels))
    ..add(ArchiveFile.string('word/document.xml', document));
  return Uint8List.fromList(ZipEncoder().encode(archive));
}
