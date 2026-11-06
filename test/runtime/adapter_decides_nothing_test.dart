// Adapter guard — the platform lease adapters decide nothing.
//
// Every routing decision lives in the shared Router. A platform
// adapter that peeks at ops or handles has stolen a decision — the
// platforms WILL drift apart from there. Parses the adapter sources
// from disk; the worker-side twin of this guard lives in
// test/runtime/web/executor_worker_sync_test.dart.

@TestOn('vm')
library;

// io-exempt: reads adapter sources from disk to verify the adapters decide nothing.
import 'dart:io';

import 'package:test/test.dart';

void main() {
  group('platform lease adapters decide nothing', () {
    // Every routing decision lives in the shared Router. A platform
    // adapter that peeks at ops or handles has stolen a decision —
    // the platforms WILL drift apart from there.
    const adapters = [
      'lib/src/runtime/native/lease.dart',
      'lib/src/runtime/native/host.dart',
      'lib/src/runtime/web/lease.dart',
      'lib/src/runtime/web/host.dart',
    ];

    for (final path in adapters) {
      test('$path makes no routing decisions', () {
        final source = File(path).readAsStringSync();
        for (final word in [
          'peekRequestOp',
          'peekRequestHandleId',
          'peekResponseHandleId',
          'docDispose',
          'editorDispose',
          'builderDispose',
          '_pins',
          'leastLoaded',
        ]) {
          expect(
            source.contains(word),
            isFalse,
            reason: 'routing logic ($word) belongs in the shared Router',
          );
        }
      });
    }
  });
}
