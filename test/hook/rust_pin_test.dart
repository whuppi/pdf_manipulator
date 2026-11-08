// The Rust pin in tool/versions.env names a release. The hook throws below
// it, builds quietly on it, and warns above it; a patch of the pinned
// release is the release.

import 'package:pdf_manipulator/src/hook/engine_compiler.dart';
import 'package:test/test.dart';

void main() {
  test('a patch of the pinned release is the release', () {
    expect(compareRustToPin('1.93.0', '1.93'), 0);
    expect(compareRustToPin('1.93.1', '1.93'), 0);
  });

  test('an older release is below the pin', () {
    expect(compareRustToPin('1.92.9', '1.93'), lessThan(0));
    expect(compareRustToPin('1.84.0', '1.93'), lessThan(0));
  });

  test('a newer release is above the pin', () {
    expect(compareRustToPin('1.94.0', '1.93'), greaterThan(0));
    expect(compareRustToPin('2.0.0', '1.93'), greaterThan(0));
  });

  test('a pin with a patch compares the patch', () {
    expect(compareRustToPin('1.93.0', '1.93.1'), lessThan(0));
    expect(compareRustToPin('1.93.1', '1.93.1'), 0);
  });

  test('a version that does not parse gives no answer', () {
    expect(compareRustToPin('nightly', '1.93'), isNull);
  });
}
