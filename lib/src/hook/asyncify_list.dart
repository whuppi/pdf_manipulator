// The wasm call graph behind the Asyncify instrumentation list.
//
// binaryen's `--asyncify` instruments exactly the functions it is told to
// (`asyncify-onlylist`); a function left out of that list must never be on the
// stack at a pause. [asyncifyList] computes the list that is safe to give: the
// callers of the pausing imports, transitively, where a `call_indirect`
// reaches every function of its signature that an element segment or a
// `ref.func` names. [WasmCallGraph.parse] refuses a module whose function
// tables could change at run time (`table.set`, `table.init/copy/grow/fill`
// on a funcref table, an imported table), because the list would then be
// unsound. A table of host values (wasm-bindgen's externref table) may change.
//
// [verifyAsyncifyList] is the wall: the build recomputes the list from the
// wasm it is about to instrument and refuses a list that is missing a
// function the wasm can have on the stack. The module has no name section, and
// binaryen names an unnamed defined function by its index among the defined
// functions, imports not counted, so the list is those indices.

import 'dart:typed_data';

/// The imports the executor worker suspends: the source read and the sink
/// write. `host_cancelled` is polled and never suspends.
const asyncifyPauseImports = ['host_read_at', 'host_write_chunk'];

/// The comma-separated `module.name` of the imports of [wasm] that pause, for
/// `asyncify-imports`.
String asyncifyImports(Uint8List wasm) {
  final graph = WasmCallGraph.parse(wasm);
  return [
    for (final i in graph.importsNamed(asyncifyPauseImports).toList()..sort())
      graph.importNames[i],
  ].join(',');
}

/// The comma-separated names to give `asyncify-onlylist`, from the engine wasm
/// [wasm]: each listed function's index among the defined functions, which is
/// the name binaryen gives a function the wasm does not name.
String asyncifyList(Uint8List wasm) {
  final graph = WasmCallGraph.parse(wasm);
  return _format(graph, graph.stackFunctions(asyncifyPauseImports));
}

/// Throws [StateError] unless [list] (as [asyncifyList] writes it) names every
/// function that can be on the stack at a pause of [wasm]; the message names
/// the functions missing.
void verifyAsyncifyList(Uint8List wasm, String list) {
  final graph = WasmCallGraph.parse(wasm);
  final listed = {
    for (final part in list.split(','))
      if (part.trim().isNotEmpty) int.parse(part.trim()) + graph.importCount,
  };
  final missing = graph
      .stackFunctions(asyncifyPauseImports)
      .where((f) => f >= graph.importCount)
      .toSet()
      .difference(listed);
  if (missing.isNotEmpty) {
    final shown = ([
      for (final f in missing) f - graph.importCount,
    ]..sort()).take(10).join(', ');
    throw StateError(
      'the asyncify list misses ${missing.length} function(s) the wasm can '
      'have on the stack at a pause (first: $shown). The list is stale or '
      'was edited; rebuild it from this wasm.',
    );
  }
}

String _format(WasmCallGraph graph, Set<int> functions) => ([
  for (final f in functions)
    if (f >= graph.importCount) f - graph.importCount,
]..sort()).join(',');

String _show(String signature) {
  const names = {0x7f: 'i32', 0x7e: 'i64', 0x7d: 'f32', 0x7c: 'f64'};
  final units = signature.codeUnits;
  final params = units[0];
  String t(int v) => names[v] ?? '0x${v.toRadixString(16)}';
  final p = [for (var i = 0; i < params; i++) t(units[1 + i])];
  final r = [for (final v in units.sublist(2 + params)) t(v)];
  return '(${p.join(',')})->(${r.join(',')})';
}

/// One signature through which the ancestor set grows.
class SignatureStep {
  /// A step of [added] functions for [signature].
  SignatureStep(this.signature, this.taken, this.sites, this.added);

  /// The signature, as `(params)->(results)`.
  final String signature;

  /// Address-taken functions of the signature that were already ancestors.
  final int taken;

  /// Functions holding a `call_indirect` of the signature.
  final int sites;

  /// Functions the signature's call sites added.
  final int added;
}

/// The call graph of a wasm module, enough to tell which functions can be on
/// the stack while an import runs.
class WasmCallGraph {
  /// Use [parse].
  WasmCallGraph({
    required this.importCount,
    required this.importNames,
    required this.callees,
    required this.indirectTypes,
    required this.funcType,
    required this.addressTaken,
  });

  /// Imported functions come first in the function index space.
  final int importCount;

  /// `module.name` of each imported function.
  final List<String> importNames;

  /// Direct callees per function.
  final List<Set<int>> callees;

  /// Per function, the signatures of its `call_indirect`s, each the bytes of the
  /// parameter and result lists of the function type.
  final List<Set<String>> indirectTypes;

  /// Signature of every function by index.
  final List<String> funcType;

  /// Functions an element segment or `ref.func` names.
  final Set<int> addressTaken;

  /// Functions in the module, imports included.
  int get functionCount => funcType.length;

  /// Indices of the imports whose `module.name` contains one of [parts]; throws
  /// [StateError] when a part names no import.
  Set<int> importsNamed(List<String> parts) {
    final found = <int>{};
    for (final part in parts) {
      final hits = [
        for (var i = 0; i < importCount; i++)
          if (importNames[i].contains(part)) i,
      ];
      if (hits.isEmpty) {
        throw StateError('no import named *$part* in $importNames');
      }
      found.addAll(hits);
    }
    return found;
  }

  List<Set<int>> _callers({required bool indirect}) {
    final callers = List.generate(funcType.length, (_) => <int>{});
    final takenByType = <String, List<int>>{};
    for (final f in addressTaken) {
      (takenByType[funcType[f]] ??= []).add(f);
    }
    for (var f = 0; f < funcType.length; f++) {
      for (final c in callees[f]) {
        callers[c].add(f);
      }
      if (!indirect) continue;
      for (final t in indirectTypes[f]) {
        for (final c in takenByType[t] ?? const <int>[]) {
          callers[c].add(f);
        }
      }
    }
    return callers;
  }

  /// The functions that can be on the stack while an import matching one of
  /// [pauseImports] runs: those imports and their callers, transitively.
  Set<int> stackFunctions(List<String> pauseImports) =>
      ancestors(importsNamed(pauseImports), indirect: true);

  /// [targets] and every function that can be on the stack while one runs;
  /// [indirect] resolves each `call_indirect` by signature.
  Set<int> ancestors(Set<int> targets, {required bool indirect}) {
    final callers = _callers(indirect: indirect);
    final seen = {...targets};
    final work = [...targets];
    while (work.isNotEmpty) {
      for (final caller in callers[work.removeLast()]) {
        if (seen.add(caller)) work.add(caller);
      }
    }
    return seen;
  }

  /// The signatures through which the ancestor set grows, in the order they
  /// are first reached from the direct callers, each with the functions it
  /// added once the call sites of that signature are counted in.
  List<SignatureStep> signatureSteps(Set<int> targets) {
    final callers = _callers(indirect: false);
    final sitesOf = <String, List<int>>{};
    for (var f = 0; f < funcType.length; f++) {
      for (final t in indirectTypes[f]) {
        (sitesOf[t] ??= []).add(f);
      }
    }
    final seen = {...targets};
    void close(List<int> work) {
      while (work.isNotEmpty) {
        for (final caller in callers[work.removeLast()]) {
          if (seen.add(caller)) work.add(caller);
        }
      }
    }

    close([...targets]);
    final steps = <SignatureStep>[];
    final done = <String>{};
    while (true) {
      final next = <String>{
        for (final f in seen)
          if (addressTaken.contains(f)) funcType[f],
      }..removeAll(done);
      if (next.isEmpty) return steps;
      for (final sig in next) {
        done.add(sig);
        final before = seen.length;
        final taken = [
          for (final f in seen)
            if (addressTaken.contains(f) && funcType[f] == sig) f,
        ].length;
        final work = <int>[];
        for (final f in sitesOf[sig] ?? const <int>[]) {
          if (seen.add(f)) work.add(f);
        }
        close(work);
        if (seen.length - before >= 100) {
          steps.add(
            SignatureStep(
              _show(sig),
              taken,
              sitesOf[sig]?.length ?? 0,
              seen.length - before,
            ),
          );
        }
      }
    }
  }

  /// Parses [bytes]. Throws [FormatException] on a binary this reader does not
  /// understand or whose tables could change at run time.
  static WasmCallGraph parse(Uint8List bytes) {
    final r = _Reader(bytes);
    if (bytes.length < 8 || r.u32le() != 0x6d736100 || r.u32le() != 1) {
      throw const FormatException('not a wasm binary');
    }
    final types = <String>[];
    final importNames = <String>[];
    final importTypes = <int>[];
    final localTypes = <int>[];
    final taken = <int>{};
    final bodies = <(int, int)>[];
    final funcTables = <int>{};

    while (!r.atEnd) {
      final id = r.byte();
      final end = r.u32() + r.pos;
      switch (id) {
        case 1:
          for (var n = r.u32(); n > 0; n--) {
            if (r.byte() != 0x60) {
              throw const FormatException('unsupported type form');
            }
            final start = r.pos;
            r.skip(r.u32());
            r.skip(r.u32());
            types.add(String.fromCharCodes(bytes.sublist(start, r.pos)));
          }
        case 2:
          for (var n = r.u32(); n > 0; n--) {
            final module = r.name();
            final field = r.name();
            switch (r.byte()) {
              case 0:
                importNames.add('$module.$field');
                importTypes.add(r.u32());
              case 1:
                throw const FormatException('imported table');
              case 2:
                final flags = r.u32();
                r.u32();
                if (flags & 1 != 0) r.u32();
              case 3:
                r.skip(2);
              default:
                throw const FormatException('import kind');
            }
          }
        case 3:
          for (var n = r.u32(); n > 0; n--) {
            localTypes.add(r.u32());
          }
        case 4:
          for (var t = 0, n = r.u32(); t < n; t++) {
            if (r.byte() == 0x70) funcTables.add(t);
            final flags = r.u32();
            r.u32();
            if (flags & 1 != 0) r.u32();
          }
        case 8:
          taken.add(r.u32());
        case 9:
          _elements(r, taken);
        case 10:
          for (var n = r.u32(); n > 0; n--) {
            final len = r.u32();
            bodies.add((r.pos, r.pos + len));
            r.skip(len);
          }
      }
      r.pos = end;
    }

    final total = importNames.length + localTypes.length;
    if (bodies.length != localTypes.length) {
      throw const FormatException('function and code counts differ');
    }
    final callees = List.generate(total, (_) => <int>{});
    final indirect = List.generate(total, (_) => <String>{});
    for (var i = 0; i < bodies.length; i++) {
      final f = importNames.length + i;
      _body(
        _Reader(bytes)..pos = bodies[i].$1,
        bodies[i].$2,
        callees[f],
        indirect[f],
        types,
        taken,
        funcTables,
      );
    }
    return WasmCallGraph(
      importCount: importNames.length,
      importNames: importNames,
      callees: callees,
      indirectTypes: indirect,
      funcType: [
        for (final t in importTypes) types[t],
        for (final t in localTypes) types[t],
      ],
      addressTaken: taken,
    );
  }

  static void _elements(_Reader r, Set<int> taken) {
    for (var n = r.u32(); n > 0; n--) {
      final flags = r.u32();
      final declared = flags & 1 != 0;
      final explicitTable = flags & 2 != 0;
      final exprs = flags & 4 != 0;
      if (!declared) {
        if (explicitTable) r.u32();
        r.constExpr(taken);
      }
      if (declared || explicitTable) r.byte();
      for (var k = r.u32(); k > 0; k--) {
        if (exprs) {
          r.constExpr(taken);
        } else {
          taken.add(r.u32());
        }
      }
    }
  }

  /// One function body: local declarations, then instructions up to [end].
  static void _body(
    _Reader r,
    int end,
    Set<int> callees,
    Set<String> indirect,
    List<String> types,
    Set<int> taken,
    Set<int> funcTables,
  ) {
    // A write to a funcref table could change what a call_indirect reaches;
    // a write to another table (wasm-bindgen's externref table) cannot.
    void refuseWrite(int table, String what) {
      if (funcTables.contains(table)) {
        throw FormatException('$what at ${r.pos}: a function table can change');
      }
    }

    for (var n = r.u32(); n > 0; n--) {
      r.u32();
      r.byte();
    }
    while (r.pos < end) {
      final op = r.byte();
      switch (op) {
        case 0x02 || 0x03 || 0x04:
          r.blockType();
        case 0x0c || 0x0d || 0x20 || 0x21 || 0x22 || 0x23 || 0x24:
          r.u32();
        case 0x25:
          r.u32();
        case 0x26:
          refuseWrite(r.u32(), 'table.set');
        case 0x0e:
          for (var n = r.u32(); n >= 0; n--) {
            r.u32();
          }
        case 0x10 || 0x12:
          callees.add(r.u32());
        case 0x11 || 0x13:
          indirect.add(types[r.u32()]);
          r.u32();
        case 0x14 || 0x15:
          throw const FormatException('call_ref is not resolved by this tool');
        case 0x1c:
          for (var n = r.u32(); n > 0; n--) {
            r.byte();
          }
        case 0xd2:
          taken.add(r.u32());
        case 0xd0:
          r.byte();
        case 0xd5 || 0xd6:
          r.u32();
        case >= 0x28 && <= 0x3e:
          r.u32();
          r.u32();
        case 0x3f || 0x40:
          r.byte();
        case 0x41 || 0x42:
          r.sleb();
        case 0x43:
          r.skip(4);
        case 0x44:
          r.skip(8);
        case 0xfc:
          _prefixFc(r, refuseWrite);
        case 0xfd:
          _prefixFd(r);
        case 0xfe:
          if (r.u32() == 3) {
            r.byte();
          } else {
            r.u32();
            r.u32();
          }
        case 0x00 ||
            0x01 ||
            0x05 ||
            0x0b ||
            0x0f ||
            0x1a ||
            0x1b ||
            0xd1 ||
            0xd4 ||
            >= 0x45 && <= 0xc4:
          break;
        default:
          throw FormatException('opcode 0x${op.toRadixString(16)}');
      }
    }
  }

  static void _prefixFc(_Reader r, void Function(int, String) refuseWrite) {
    final sub = r.u32();
    switch (sub) {
      case <= 7:
        break;
      case 8:
        r.u32();
        r.byte();
      case 9 || 13:
        r.u32();
      case 10:
        r.skip(2);
      case 11:
        r.byte();
      case 12:
        r.u32();
        refuseWrite(r.u32(), 'table.init');
      case 14:
        refuseWrite(r.u32(), 'table.copy');
        r.u32();
      case 15:
        refuseWrite(r.u32(), 'table.grow');
      case 16:
        r.u32();
      case 17:
        refuseWrite(r.u32(), 'table.fill');
      default:
        throw FormatException('0xfc sub-opcode $sub');
    }
  }

  static void _prefixFd(_Reader r) {
    final sub = r.u32();
    switch (sub) {
      case <= 11 || 92 || 93:
        r.u32();
        r.u32();
      case 12 || 13:
        r.skip(16);
      case >= 21 && <= 34:
        r.byte();
      case >= 84 && <= 91:
        r.u32();
        r.u32();
        r.byte();
      default:
        break;
    }
  }
}

class _Reader {
  _Reader(this.b);
  final Uint8List b;
  int pos = 0;

  bool get atEnd => pos >= b.length;

  int byte() => b[pos++];

  void skip(int n) => pos += n;

  int u32le() {
    final v = ByteData.sublistView(b).getUint32(pos, Endian.little);
    pos += 4;
    return v;
  }

  int u32() {
    var result = 0;
    var shift = 0;
    while (true) {
      final x = b[pos++];
      result |= (x & 0x7f) << shift;
      if (x & 0x80 == 0) return result;
      shift += 7;
    }
  }

  void sleb() {
    while (b[pos++] & 0x80 != 0) {}
  }

  void blockType() {
    final x = b[pos];
    if (x == 0x40 || x >= 0x6f && x <= 0x7f) {
      pos++;
    } else {
      sleb();
    }
  }

  String name() {
    final n = u32();
    final s = String.fromCharCodes(b.sublist(pos, pos + n));
    pos += n;
    return s;
  }

  /// A constant expression to its `end`; a `ref.func` names an
  /// address-taken function.
  void constExpr(Set<int> taken) {
    while (true) {
      final op = byte();
      switch (op) {
        case 0x0b:
          return;
        case 0xd2:
          taken.add(u32());
        case 0x41 || 0x42:
          sleb();
        case 0x23:
          u32();
        case 0xd0:
          byte();
        case 0x43:
          skip(4);
        case 0x44:
          skip(8);
        default:
          throw FormatException('constant expression opcode $op');
      }
    }
  }
}
