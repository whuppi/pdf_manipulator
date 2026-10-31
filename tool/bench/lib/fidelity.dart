import 'dart:math' as math;
import 'dart:typed_data';

import 'package:image/image.dart' as img;
import 'package:pdf_manipulator/pdf_manipulator.dart';

/// Render fidelity of one document against its source.
class Fidelity {
  const Fidelity({
    required this.pages,
    required this.worstRmse,
    required this.worstRmsePage,
    required this.worstSsim,
    required this.worstSsimPage,
    required this.textEqual,
  });

  /// Pages compared.
  final int pages;

  /// Largest per-page RMSE over the RGB bytes (0 = pixel-identical).
  final double worstRmse;
  final int worstRmsePage;

  /// Smallest per-page mean SSIM over 8x8 luma blocks (1 = identical).
  final double worstSsim;
  final int worstSsimPage;

  /// Whether the extracted text of the whole document is equal.
  final bool textEqual;
}

/// The fidelity of one candidate, or the error that stopped its comparison.
typedef Outcome = ({Fidelity? fidelity, String? error});

/// Renders every page of [source] and of each of [candidates] at [dpi] with
/// the package's own renderer and compares them page by page, so only one
/// page of bitmaps is alive at a time. Each source page is rendered once,
/// and decoded only when some candidate's render differs from it. [maxPages]
/// of 0 compares every page. A candidate that fails gets an error outcome
/// and does not stop the others.
Future<List<Outcome>> compareAll(
  PdfDoc source,
  List<PdfDoc> candidates, {
  int dpi = 100,
  int maxPages = 0,
}) async {
  final n = candidates.length;
  final limits = [
    for (final c in candidates)
      () {
        final count = math.min(source.pageCount, c.pageCount);
        return maxPages > 0 ? math.min(count, maxPages) : count;
      }(),
  ];
  final errors = List<String?>.filled(n, null);
  final worstRmse = List<double>.filled(n, 0);
  final worstRmsePage = List<int>.filled(n, 0);
  final worstSsim = List<double>.filled(n, 1);
  final worstSsimPage = List<int>.filled(n, 0);
  final longest = limits.fold(0, math.max);

  for (var i = 0; i < longest; i++) {
    final size = _sizeFor(source.pages[i], dpi);
    final sourcePng = await _render(source, i, size);
    img.Image? decoded;
    for (var k = 0; k < n; k++) {
      if (errors[k] != null || i >= limits[k]) continue;
      try {
        final png = await _render(candidates[k], i, size);
        var rmse = 0.0;
        var ssim = 1.0;
        if (!_equal(sourcePng, png)) {
          final a = decoded ??= _decode(sourcePng, i);
          final pair = _score(a, _decode(png, i));
          rmse = pair.rmse;
          ssim = pair.ssim;
        }
        if (rmse > worstRmse[k]) {
          worstRmse[k] = rmse;
          worstRmsePage[k] = i;
        }
        if (ssim < worstSsim[k]) {
          worstSsim[k] = ssim;
          worstSsimPage[k] = i;
        }
      } on Object catch (e) {
        errors[k] = e.toString().split('\n').first;
      }
    }
  }

  final textA = await source.extract(pages: const PdfPages.all());
  final outcomes = <Outcome>[];
  for (var k = 0; k < n; k++) {
    if (errors[k] == null) {
      try {
        final textB = await candidates[k].extract(pages: const PdfPages.all());
        outcomes.add((
          fidelity: Fidelity(
            pages: limits[k],
            worstRmse: worstRmse[k],
            worstRmsePage: worstRmsePage[k],
            worstSsim: worstSsim[k],
            worstSsimPage: worstSsimPage[k],
            textEqual: textA == textB,
          ),
          error: null,
        ));
        continue;
      } on Object catch (e) {
        errors[k] = e.toString().split('\n').first;
      }
    }
    outcomes.add((fidelity: null, error: errors[k]));
  }
  return outcomes;
}

PdfRenderSize _sizeFor(PdfPageInfo page, int dpi) {
  // The box is square on the longer side, so a rotated page is scaled by
  // the same factor as an upright one.
  final side = (math.max(page.width, page.height) * dpi / 72).ceil();
  return PdfRenderSize.fit(maxWidth: side, maxHeight: side);
}

Future<Uint8List> _render(PdfDoc doc, int page, PdfRenderSize size) async {
  final sink = MemorySink();
  await doc.render((_) => sink, pages: PdfPages.single(page), size: size);
  return sink.takeBytes();
}

img.Image _decode(Uint8List png, int page) {
  final decoded = img.decodePng(png);
  if (decoded == null) throw StateError('page $page: render is not a PNG');
  return decoded;
}

bool _equal(Uint8List a, Uint8List b) {
  if (a.length != b.length) return false;
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}

({double rmse, double ssim}) _score(img.Image a, img.Image b) {
  if (a.width != b.width || a.height != b.height) {
    return (rmse: double.infinity, ssim: 0);
  }
  final ba = a.getBytes(order: img.ChannelOrder.rgb);
  final bb = b.getBytes(order: img.ChannelOrder.rgb);
  var sum = 0.0;
  for (var i = 0; i < ba.length; i++) {
    final d = ba[i] - bb[i];
    sum += d * d;
  }
  final rmse = math.sqrt(sum / ba.length);
  return (rmse: rmse, ssim: rmse == 0 ? 1 : _ssim(ba, bb, a.width, a.height));
}

/// Mean SSIM over non-overlapping 8x8 blocks of the Rec. 601 luma.
double _ssim(Uint8List a, Uint8List b, int width, int height) {
  const c1 = 6.5025; // (0.01 * 255)^2
  const c2 = 58.5225; // (0.03 * 255)^2
  double luma(Uint8List p, int x, int y) {
    final o = (y * width + x) * 3;
    return 0.299 * p[o] + 0.587 * p[o + 1] + 0.114 * p[o + 2];
  }

  var total = 0.0;
  var blocks = 0;
  for (var y = 0; y + 8 <= height; y += 8) {
    for (var x = 0; x + 8 <= width; x += 8) {
      var ma = 0.0;
      var mb = 0.0;
      for (var j = 0; j < 8; j++) {
        for (var i = 0; i < 8; i++) {
          ma += luma(a, x + i, y + j);
          mb += luma(b, x + i, y + j);
        }
      }
      ma /= 64;
      mb /= 64;
      var va = 0.0;
      var vb = 0.0;
      var cov = 0.0;
      for (var j = 0; j < 8; j++) {
        for (var i = 0; i < 8; i++) {
          final da = luma(a, x + i, y + j) - ma;
          final db = luma(b, x + i, y + j) - mb;
          va += da * da;
          vb += db * db;
          cov += da * db;
        }
      }
      va /= 63;
      vb /= 63;
      cov /= 63;
      total +=
          ((2 * ma * mb + c1) * (2 * cov + c2)) /
          ((ma * ma + mb * mb + c1) * (va + vb + c2));
      blocks++;
    }
  }
  return blocks == 0 ? 1 : total / blocks;
}
