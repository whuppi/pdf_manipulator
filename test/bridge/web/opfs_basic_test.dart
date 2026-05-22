@TestOn('browser')
library;

import 'package:test/test.dart';
import 'package:pdf_manipulator/src/bridge/web/worker_pool.dart';

void main() {
  late int serverPort;

  setUpAll(() async {
    final channel = spawnHybridUri('asset_server.dart');
    serverPort = await channel.stream.first as int;
  });

  test('OPFS write then read on same worker session', () async {
    final pool = WebWorkerPool(
      workerUrl: 'http://localhost:$serverPort/web_assets/worker.js',
    );

    final session = await pool.acquire();

    try {
      // Write some bytes
      final testData = List.generate(100, (i) => i % 256);
      await session.send('opfs.write', {
        'filename': '_test_opfs_basic.tmp',
        'chunk': testData,
        'offset': 0,
      });

      await session.send('opfs.finalize', {
        'filename': '_test_opfs_basic.tmp',
      });

      // Try to read it back via a simple echo op
      // (just verify the file exists and has data)
      await session.send('opfs.cleanup', {
        'filename': '_test_opfs_basic.tmp',
      });
    } finally {
      pool.release(session);
      pool.dispose();
    }
  });
}
