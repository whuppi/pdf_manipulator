import 'dart:io';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:toml/toml.dart';

/// One manifest row: a public, redistributable PDF pinned by SHA-256.
class CorpusEntry {
  const CorpusEntry({
    required this.id,
    required this.url,
    required this.sha256,
    required this.license,
    required this.licenseUrl,
    required this.kind,
    required this.note,
  });

  final String id;
  final String url;
  final String sha256;
  final String license;
  final String licenseUrl;
  final String kind;
  final String note;
}

/// Reads corpus.toml and downloads its files into [cacheDir] on demand.
class Corpus {
  Corpus({required this.manifest, required this.cacheDir});

  final File manifest;
  final Directory cacheDir;

  List<CorpusEntry> entries() {
    final doc = TomlDocument.parse(manifest.readAsStringSync()).toMap();
    return [
      for (final row in doc['file'] as List<dynamic>)
        CorpusEntry(
          id: row['id'] as String,
          url: row['url'] as String,
          sha256: row['sha256'] as String,
          license: row['license'] as String,
          licenseUrl: row['license_url'] as String,
          kind: row['kind'] as String,
          note: row['note'] as String,
        ),
    ];
  }

  /// Returns the verified bytes of [entry], downloading when the cached
  /// copy is missing or its hash differs. Throws when the hash on the wire
  /// does not match the manifest.
  Future<Uint8List> bytes(CorpusEntry entry) async {
    final file = File('${cacheDir.path}/${entry.id}.pdf');
    if (file.existsSync()) {
      final cached = file.readAsBytesSync();
      if (sha256.convert(cached).toString() == entry.sha256) return cached;
    }
    cacheDir.createSync(recursive: true);
    final fetched = await _download(entry.url);
    final digest = sha256.convert(fetched).toString();
    if (digest != entry.sha256) {
      throw StateError(
        '${entry.id}: sha256 $digest, manifest says '
        '${entry.sha256}',
      );
    }
    file.writeAsBytesSync(fetched);
    return fetched;
  }

  static Future<Uint8List> _download(String url) async {
    final client = HttpClient()
      ..userAgent =
          'pdf_manipulator-bench/1.0 '
          '(+https://github.com/whuppi/pdf_manipulator)';
    try {
      final response = await (await client.getUrl(Uri.parse(url))).close();
      if (response.statusCode != 200) {
        await response.drain<void>();
        throw HttpException('HTTP ${response.statusCode}', uri: Uri.parse(url));
      }
      final builder = BytesBuilder(copy: false);
      await for (final chunk in response) {
        builder.add(chunk);
      }
      return builder.takeBytes();
    } finally {
      client.close();
    }
  }
}
