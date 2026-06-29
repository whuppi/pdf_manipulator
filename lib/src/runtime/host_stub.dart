// Stub conditional-import target for host.dart's createLaneHost. Carries NO
// platform library on purpose: the default import is the baseline pub.dev's
// analyzer attributes to EVERY platform, so a dart:io/ffi default would mark
// the whole package native-only and drop web. Real platforms resolve to the
// native or web lane library; this throws only where neither exists. Do not
// make a concrete platform library the default.

import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';

/// Unreachable on a real platform — the conditional import resolves to the
/// native or web runtime there. Present so the default import is neutral.
LaneHost createLaneHost({PdfConfig? config}) =>
    throw UnsupportedError('pdf_manipulator: no lane runtime for this platform');
