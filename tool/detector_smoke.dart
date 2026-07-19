// Manual smoke: runs the trim detector over example/ and prints the
// keep-set. Not part of any gate; kept for quick detector verification.
import 'package:pdf_manipulator/src/trim/detector.dart';

void main() {
  final r = detectCapabilities('example');
  final caps = r.keep.map((c) => c.wire).toList()..sort();
  print('resolved=${r.resolved} keep=$caps');
  print('matched: ${describeMatches(r)}');
}
