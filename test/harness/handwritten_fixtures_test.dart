// Every hand-typed cross-reference table in test/fixtures/handwritten.dart
// points where it says. The engine repairs a wrong offset without a word,
// so a stale `startxref` or entry would let a test run on a repaired file
// and never notice; the incremental save refuses repaired sources, which
// is how the last one was found.

// io-exempt: the source-scan wall reads handwritten.dart's own text to
// assert every valid fixture builds through _offsetPdf.
import 'dart:io';
import 'dart:typed_data';

import 'package:test/test.dart';

import '../fixtures/handwritten.dart';

/// Why [pdf]'s last cross-reference section does not describe it, or null.
String? xrefProblem(Uint8List pdf) {
  final text = String.fromCharCodes(pdf); // bytegrep-exempt: file structure
  final at = text.lastIndexOf('startxref');
  if (at < 0) return 'no startxref';
  final m = RegExp(r'startxref\s+(\d+)').matchAsPrefix(text, at);
  if (m == null) return 'unreadable startxref';
  final offset = int.parse(m.group(1)!);
  if (offset >= text.length) return 'startxref $offset is past the end';
  final section = text.substring(offset);
  if (RegExp(r'^\d+\s+\d+\s+obj').hasMatch(section)) return null;
  if (!section.startsWith('xref')) return 'startxref $offset is not a section';
  final lines = section.split('\n');
  var i = 1;
  while (i < lines.length) {
    final head = RegExp(r'^(\d+) (\d+)$').firstMatch(lines[i].trim());
    if (head == null) break;
    final first = int.parse(head.group(1)!);
    final count = int.parse(head.group(2)!);
    for (var k = 0; k < count; k++) {
      final entry = lines[i + 1 + k];
      if (entry.substring(17, 18) != 'n') continue;
      final at = int.parse(entry.substring(0, 10));
      final object = RegExp('^${first + k}\\s+\\d+\\s+obj');
      if (!object.hasMatch(text.substring(at))) {
        return 'object ${first + k} is not at $at';
      }
    }
    i += count + 1;
  }
  return null;
}

void main() {
  final fixtures = <String, Uint8List>{
    'minimalPdf': minimalPdf,
    'recoverProbePdf': recoverProbePdf,
    'leakProbePdf': leakProbePdf,
    'letterPdf': letterPdf,
    'indirectAnnotsForm': indirectAnnotsForm,
    'imageAndFormPdf': imageAndFormPdf,
    'imageInFormPdf': imageInFormPdf,
    'sharedImagePdf': sharedImagePdf,
    'separationImagePdf': separationImagePdf,
    'colorKeyMaskPdf': colorKeyMaskPdf,
    'stencilImageMaskPdf': stencilImageMaskPdf,
    'jbig2StubPdf': jbig2StubPdf,
    'bilevelRawPdf': bilevelRawPdf,
    'gray16RawPdf': gray16RawPdf,
    'croppedPagePdf': croppedPagePdf,
    'openActionJavaScriptPdf': openActionJavaScriptPdf,
    'xfaFormPdf': xfaFormPdf,
    'attachmentPdf': attachmentPdf,
    'emptyApTextForm': emptyApTextForm,
    'uncheckedButtonForm': uncheckedButtonForm,
    'yesNoRadioForm': yesNoRadioForm,
    'inheritedFieldAttrsForm': inheritedFieldAttrsForm,
    'bookmarkedPdf': bookmarkedPdf,
    'formPdfdocNamePdf': formPdfdocNamePdf,
    'formUtf16NamePdf': formUtf16NamePdf,
    'formUtf8NamePdf': formUtf8NamePdf,
    'pagePrunePdf': pagePrunePdf,
    'certifiedPdf(2)': certifiedPdf(2),
    'structTreePdf': structTreePdf,
    'noSizePdf': noSizePdf,
    'freeEntryPdf': freeEntryPdf,
    'genOnePdf': genOnePdf,
    // brokenObjectPdf stays out: its xref entry deliberately does not point
    // where it says (object 4 is nowhere in the file), which is the damage
    // the fixture carries, not a fixture bug.
  };
  for (final MapEntry(key: name, value: pdf) in fixtures.entries) {
    test('$name: its xref points where it says', () {
      expect(xrefProblem(pdf), isNull);
    });
  }

  test('staleStartxrefPdf is stale on purpose', () {
    expect(xrefProblem(staleStartxrefPdf), contains('not a section'));
  });

  // Every VALID hand-built fixture must come from _offsetPdf, whose
  // offsets are correct by construction — a hand-typed table can drift
  // as the file grows. _build stays only inside the two deliberately
  // broken declarations.
  test('handwritten.dart builds valid fixtures only through _offsetPdf', () {
    final source = File('test/fixtures/handwritten.dart').readAsStringSync();
    for (final match in RegExp('= _build\\(').allMatches(source)) {
      // The call's own line names its declaration, up to the '='.
      final lineStart = source.lastIndexOf('\n', match.start - 2) + 1;
      final declLine = source.substring(lineStart, match.start).trim();
      final owner = RegExp(r'^final .*?(\w+)\s*$').firstMatch(declLine);
      final name = owner?.group(1) ?? declLine;
      expect(
        const {'staleStartxrefPdf', 'brokenPdf'}.contains(name),
        isTrue,
        reason:
            'fixture "$name" hand-builds its bytes; '
            'only deliberate damage may use _build — every valid fixture '
            'goes through _offsetPdf',
      );
    }
  });
}
