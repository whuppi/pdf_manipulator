// The transfer budget is a share of the keep line: copies of source bytes
// in a worker must never be what retires it.

import 'package:pdf_manipulator/src/runtime/pool.dart' show kKeepBytes;
import 'package:pdf_manipulator/src/runtime/web/protocol.dart'
    show kTransferBytes;
import 'package:test/test.dart';

void main() {
  test('the transfer budget is at most half the keep line', () {
    expect(kTransferBytes, lessThanOrEqualTo(kKeepBytes ~/ 2));
    expect(kTransferBytes, greaterThan(0));
  });
}
