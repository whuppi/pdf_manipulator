// Web test helper — creates same-origin blob Workers from cross-origin scripts.
//
// dart test -p chrome runs tests on localhost:PORT/_HASH_/test/...
// Worker('http://asset-server/coordinator.js') fails with SecurityError
// because Workers require same-origin scripts.
//
// Solution: fetch the script text from the asset server, create a Blob URL,
// construct the Worker from the blob. The blob is same-origin.

import 'dart:js_interop';
import 'package:web/web.dart' as web;

/// Fetch a JS file from [url] and return a same-origin blob URL.
Future<String> fetchAsBlobUrl(String url) async {
  final response = await web.window.fetch(url.toJS).toDart;
  if (!response.ok) {
    throw Exception('Failed to fetch $url: ${response.status}');
  }
  final text = (await response.text().toDart).toDart;
  final blob = web.Blob(
    [text.toJS].toJS,
    web.BlobPropertyBag(type: 'application/javascript'),
  );
  return web.URL.createObjectURL(blob);
}
