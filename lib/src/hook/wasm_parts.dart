// The web wasm as parts: every module the package ships is cut into parts of
// wasmPartBytes, listed with their hashes in pdf_oxide_manifest.json. Every
// producer (`make build-wasm`, the release, the hook's from-source build) cuts
// through writeWasmParts, so there is one cut and one manifest shape. The
// browser side, executor_worker.js and host.dart, joins the parts it reads from
// the manifest into one stream while it compiles.

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:path/path.dart' as p;

import 'build_constants.dart';

/// The manifest file beside `executor_worker.js`.
const wasmManifestFile = 'pdf_oxide_manifest.json';

/// The module every mode but Asyncify runs.
const normalWasmModule = 'pdf_oxide_bg';

/// The Asyncify engine's module: shipped only to apps that set
/// `asyncify: true`, and fetched by a browser only in Asyncify mode.
const asyncifyWasmModule = 'pdf_oxide_asyncify_bg';

const _moduleOrder = [normalWasmModule, asyncifyWasmModule];

/// One file of a cut module.
class WasmPart {
  /// A part as the manifest lists it.
  const WasmPart({
    required this.file,
    required this.bytes,
    required this.sha256,
  });

  /// Reads a part from its manifest JSON.
  factory WasmPart.fromJson(Map<String, dynamic> json) => WasmPart(
    file: json['file'] as String,
    bytes: json['bytes'] as int,
    sha256: json['sha256'] as String,
  );

  /// The part's file name beside the manifest.
  final String file;

  /// The part's size.
  final int bytes;

  /// The part's SHA-256, in hex.
  final String sha256;

  /// The part as manifest JSON.
  Map<String, Object> toJson() => {
    'file': file,
    'bytes': bytes,
    'sha256': sha256,
  };
}

/// One cut module: its size and hash as a whole, and its parts in order.
class WasmModule {
  /// A module as the manifest lists it.
  const WasmModule({
    required this.bytes,
    required this.sha256,
    required this.parts,
  });

  /// Reads a module from its manifest JSON.
  factory WasmModule.fromJson(Map<String, dynamic> json) => WasmModule(
    bytes: json['bytes'] as int,
    sha256: json['sha256'] as String,
    parts: [
      for (final part in json['parts'] as List)
        WasmPart.fromJson(part as Map<String, dynamic>),
    ],
  );

  /// The module's size as a whole.
  final int bytes;

  /// The SHA-256 of the whole module, in hex.
  final String sha256;

  /// The parts, in the order they join.
  final List<WasmPart> parts;

  /// The module as manifest JSON.
  Map<String, Object> toJson() => {
    'bytes': bytes,
    'sha256': sha256,
    'parts': [for (final part in parts) part.toJson()],
  };
}

/// `pdf_oxide_manifest.json`: the modules a web build shipped, by name.
class WasmManifest {
  /// A manifest of [modules].
  const WasmManifest(this.modules);

  /// The manifest in [dir]; throws when it is missing or not a manifest.
  factory WasmManifest.load(Directory dir) {
    final file = File(p.join(dir.path, wasmManifestFile));
    final json = jsonDecode(file.readAsStringSync()) as Map<String, dynamic>;
    return WasmManifest({
      for (final MapEntry(:key, :value) in json.entries)
        key: WasmModule.fromJson(value as Map<String, dynamic>),
    });
  }

  /// The manifest in [dir], or an empty one when none is there yet.
  factory WasmManifest.loadOrEmpty(Directory dir) =>
      File(p.join(dir.path, wasmManifestFile)).existsSync()
      ? WasmManifest.load(dir)
      : const WasmManifest({});

  /// The modules by name, in the order the file lists them.
  final Map<String, WasmModule> modules;

  /// Every file the manifest lists, in manifest order.
  List<String> get files => [
    for (final module in modules.values)
      for (final part in module.parts) part.file,
  ];

  /// The same manifest with [module] set to [entry], in the fixed module order.
  WasmManifest withModule(String module, WasmModule entry) {
    final merged = {...modules, module: entry};
    return WasmManifest({
      for (final name in _moduleOrder)
        if (merged.containsKey(name)) name: merged[name]!,
      for (final MapEntry(:key, :value) in merged.entries)
        if (!_moduleOrder.contains(key)) key: value,
    });
  }

  /// The same manifest without [module].
  WasmManifest without(String module) =>
      WasmManifest({...modules}..remove(module));

  /// Stores the manifest in [dir].
  void storeIn(Directory dir) {
    File(p.join(dir.path, wasmManifestFile)).writeAsStringSync(
      '${const JsonEncoder.withIndent('  ').convert({for (final MapEntry(:key, :value) in modules.entries) key: value.toJson()})}\n',
    );
  }
}

String _hex(List<int> bytes) => sha256.convert(bytes).toString();

RegExp _partName(String module) =>
    RegExp('^${RegExp.escape(module)}\\.\\d+\\.wasm\$');

void _deleteParts(Directory dir, String module) {
  final name = _partName(module);
  for (final entity in dir.listSync().whereType<File>()) {
    if (name.hasMatch(p.basename(entity.path))) entity.deleteSync();
  }
}

/// Cuts [wasm] into parts of exactly [wasmPartBytes] (the last one shorter)
/// named `<module>.<n>.wasm` in [outDir], and merges the module's entry into
/// the manifest there, keeping the other modules'. The parts the module had
/// before, however many, are replaced. Returns the merged manifest.
WasmManifest writeWasmParts({
  required File wasm,
  required String module,
  required Directory outDir,
}) {
  final bytes = wasm.readAsBytesSync();
  outDir.createSync(recursive: true);
  _deleteParts(outDir, module);
  final parts = <WasmPart>[];
  for (
    var start = 0, n = 0;
    start < bytes.length;
    start += wasmPartBytes, n++
  ) {
    final end = start + wasmPartBytes < bytes.length
        ? start + wasmPartBytes
        : bytes.length;
    final slice = Uint8List.sublistView(bytes, start, end);
    final file = '$module.$n.wasm';
    File(p.join(outDir.path, file)).writeAsBytesSync(slice);
    parts.add(WasmPart(file: file, bytes: slice.length, sha256: _hex(slice)));
  }
  final manifest = WasmManifest.loadOrEmpty(outDir).withModule(
    module,
    WasmModule(bytes: bytes.length, sha256: _hex(bytes), parts: parts),
  );
  manifest.storeIn(outDir);
  return manifest;
}

/// Deletes [module]'s parts from [dir] and its entry from the manifest there.
void removeWasmModule(Directory dir, String module) {
  _deleteParts(dir, module);
  final manifest = WasmManifest.loadOrEmpty(dir).without(module);
  if (manifest.modules.isEmpty) {
    final file = File(p.join(dir.path, wasmManifestFile));
    if (file.existsSync()) file.deleteSync();
  } else {
    manifest.storeIn(dir);
  }
}

/// The module [entry] lists, joined from its parts in [dir].
Uint8List joinWasmParts(Directory dir, WasmModule entry) {
  final out = BytesBuilder(copy: false);
  for (final part in entry.parts) {
    out.add(File(p.join(dir.path, part.file)).readAsBytesSync());
  }
  return out.takeBytes();
}

/// What is wrong with the parts in [dir] against [manifest], one line each;
/// empty when every part is as listed and every module joins to its listed
/// size and hash.
List<String> verifyWasmParts(Directory dir, WasmManifest manifest) {
  final problems = <String>[];
  for (final MapEntry(key: name, value: module) in manifest.modules.entries) {
    final joined = BytesBuilder(copy: false);
    var intact = true;
    for (final part in module.parts) {
      final file = File(p.join(dir.path, part.file));
      if (!file.existsSync()) {
        problems.add('${part.file}: the manifest lists it and it is missing');
        intact = false;
        continue;
      }
      final bytes = file.readAsBytesSync();
      if (bytes.length != part.bytes) {
        problems.add(
          '${part.file}: ${bytes.length} bytes, the manifest lists ${part.bytes}',
        );
        intact = false;
      } else if (_hex(bytes) != part.sha256) {
        problems.add('${part.file}: its sha256 differs from the manifest');
        intact = false;
      }
      joined.add(bytes);
    }
    if (!intact) continue;
    final whole = joined.takeBytes();
    if (whole.length != module.bytes) {
      problems.add(
        '$name: the parts join to ${whole.length} bytes, the manifest lists ${module.bytes}',
      );
    } else if (_hex(whole) != module.sha256) {
      problems.add('$name: the parts join to a different sha256');
    }
  }
  return problems;
}

/// The gzip size of [module] as a whole, which is what a visitor downloads.
int gzipBytesOf(Uint8List module) => gzip.encode(module).length;

/// What breaks the wall around the web wasm in [dir], one line each; empty
/// when none does. The wall: every file is under [maxFileBytes], every part
/// and module is as the manifest lists it, and each module's gzip size is
/// within its budget (a module with no budget has none to break).
List<String> wasmWallProblems(
  Directory dir, {
  int maxFileBytes = maxWebAssetBytes,
  Map<String, int> gzipBudgets = const {
    normalWasmModule: normalWasmGzipBudget,
    asyncifyWasmModule: asyncifyWasmGzipBudget,
  },
}) {
  final problems = <String>[];
  for (final file in dir.listSync().whereType<File>()) {
    if (file.lengthSync() >= maxFileBytes) {
      problems.add(
        '${p.basename(file.path)}: ${file.lengthSync()} bytes reaches the '
        '$maxFileBytes-byte limit of a file',
      );
    }
  }
  final manifest = WasmManifest.load(dir);
  problems.addAll(verifyWasmParts(dir, manifest));
  if (problems.isNotEmpty) return problems;
  for (final MapEntry(key: name, value: module) in manifest.modules.entries) {
    final budget = gzipBudgets[name];
    if (budget == null) continue;
    final gzipped = gzipBytesOf(joinWasmParts(dir, module));
    if (gzipped > budget) {
      problems.add(
        '$name: $gzipped gzip bytes exceed the budget of $budget; raising a '
        'budget is a decision, not a fix',
      );
    }
  }
  return problems;
}

/// Where the manifest in [dir] and build.json's web list disagree: a part the
/// manifest lists that build.json does not name (the release would not ship
/// it) and, when [complete], a listed part the manifest does not have.
List<String> buildJsonPartProblems(
  Directory dir,
  BuildConstants constants, {
  bool complete = false,
}) {
  final manifest = WasmManifest.load(dir);
  final listed = {
    for (final name in constants.webAssets.keys)
      if (name.endsWith('.wasm')) name,
  };
  return [
    for (final file in manifest.files)
      if (!listed.contains(file))
        '$file: the manifest lists it and build.json\'s web list does not',
    if (complete)
      for (final name in listed)
        if (!manifest.files.contains(name))
          '$name: build.json lists it and the manifest does not',
  ];
}
