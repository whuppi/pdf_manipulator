// pdf_manipulator example — every capability, one file.
//
// ONE file on purpose — pub.dev renders example/lib/main.dart as the
// package's Example tab. Splitting it hides everything past main.dart
// from that page. Do not modularize.
//
// Seven tabs, one per API surface:
//   Runtime    — the lane architecture: I/O mode, parallel ops,
//                per-op cancellation, instant dispose
//   Doc        — PdfDoc read-only queries
//   Sugar      — one-shot convenience ops (PdfSugar)
//   Standalone — source in → sink out, no handle (PdfStandalone)
//   Editor     — parse once, mutate N times, save once (PdfEditor)
//   Builder    — create PDFs from scratch (PdfBuilder)
//   Merge      — reorderable multi-file merge
//
// Every engine method returns a PdfTask — a Future you can cancel.
// The status bar shows a Cancel button whenever a cancellable task
// is in flight, on every tab.

import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show rootBundle;

import 'package:pdf_manipulator/pdf_manipulator.dart';

// ─── Custom DataSource / DataSink — the real-app pattern ──────────
//
// A real app's bytes rarely sit in a Uint8List — they live in files,
// network streams, pickers, your own store. Implementing the two tiny
// interfaces is how you wire any of those in. These are this example's
// own implementations, used throughout the app.
//
// For the quick path, the package SHIPS MemorySource / MemorySink
// (`package:pdf_manipulator/pdf_manipulator.dart`) and FileSource /
// FileSink (`package:pdf_manipulator/io.dart`). The Sugar tab shows
// the shipped MemorySink in use; everything else uses these custom
// impls to show the interface is open.

/// Random-access source over bytes already in memory.
class DemoSource implements DataSource {
  DemoSource(this._data);
  final Uint8List _data;

  @override
  int get length => _data.length;

  @override
  Uint8List readAt(int offset, int count) {
    if (offset >= _data.length) return Uint8List(0);
    final end = (offset + count).clamp(0, _data.length);
    return Uint8List.sublistView(_data, offset, end);
  }
}

/// Collects output chunks; hand the bytes off with [takeBytes].
class DemoSink implements DataSink {
  final _builder = BytesBuilder(copy: false);

  @override
  void write(Uint8List chunk) => _builder.add(chunk);

  Uint8List takeBytes() => _builder.takeBytes();
  int get length => _builder.length;
}

// ─── Built-in fixtures (the app works without picking any file) ───

/// Minimal valid PDF: one blank A4 page.
final minimalPdf = Uint8List.fromList(
  '%PDF-1.4\n'
          '1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n'
          '2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n'
          '3 0 obj\n<< /Type /Page /Parent 2 0 R '
          '/MediaBox [0 0 595 842] >>\nendobj\n'
          'xref\n0 4\n'
          '0000000000 65535 f \n'
          '0000000009 00000 n \n'
          '0000000058 00000 n \n'
          '0000000115 00000 n \n'
          'trailer\n<< /Size 4 /Root 1 0 R >>\n'
          'startxref\n190\n%%EOF\n'
      .codeUnits,
);

/// Self-signed test certificate for the signing demo.
const testCertPem = '''
-----BEGIN CERTIFICATE-----
MIIDSTCCAjGgAwIBAgIUexfMwJDl5Rlv9CCPzlG2ZMWIrigwDQYJKoZIhvcNAQEL
BQAwNDEWMBQGA1UEAwwNcGRmb3hpZGUtdGVzdDENMAsGA1UECgwEVGVzdDELMAkG
A1UEBhMCVVMwHhcNMjYwNDI0MDg1NDA1WhcNMzYwNDIxMDg1NDA1WjA0MRYwFAYD
VQQDDA1wZGZveGlkZS10ZXN0MQ0wCwYDVQQKDARUZXN0MQswCQYDVQQGEwJVUzCC
ASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAMvG5TAigvrKPPkWY4CAoye5
SXu32oTJZkzXZWMWETPosyhUxzu1UAezQpgosBV3oH1tTq7BjWEol1dI1eYplXWF
4Rpry3meJIGkiAbBLTn64UbP886sxQvplpAYcGLeWbT6LTdCvI1BOk55w9eC1RjF
Vx/ib/YYsgHyBFXIWSpz3d+eZOFnS5PwdkUaj0zk/KTHIFIXE7GoeaAGtDkKLmfP
ZOh0HMXTRslPF8n/ls42OGPiB9nB5f6Gd4mptU6kLxmh8KTsfSTWxiqmisX2u5kO
HL4t+7Ld9Y5vJAHfAN6QMWhmI3ESzZPp9i6+MuLGOhjmnGV2Si0i/uaS6vvURPsC
AwEAAaNTMFEwHQYDVR0OBBYEFGGx/fXllkuIQcEuZQPTJV7qJ43dMB8GA1UdIwQY
MBaAFGGx/fXllkuIQcEuZQPTJV7qJ43dMA8GA1UdEwEB/wQFMAMBAf8wDQYJKoZI
hvcNAQELBQADggEBAHavjpwAV1Dq3XBEMz/+X9bGfZ8i5zfMjeywhhUUAnGSzl24
c3tKigiknq45lJ2ITZDxuLXqdG8oU549hnrGw3Ja0RdKvHSBCvOTBs0APnNX07V4
aoq9gdNQnXKynVlOeFiccvtYeu9o9OGFTttfQbpB0Dpe568YH7NhV3DxEdtsKoK+
rTUImAGg+mebrEe6ts9FV/lEwnMOJnCdvH9c215yuIWK+fCn3qcPmzWWv08oEr4w
8Xy/7D8D6MtVlWFXT3YgogusJECJUXioAai4XUI3bAoNwSTw4vwGnnA9+82Nv5qU
2YWiHsI5E2QSTEFR4Njsmjjrj0FkQtyKdDBOQKs=
-----END CERTIFICATE-----''';

const testKeyPem = '''
-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDLxuUwIoL6yjz5
FmOAgKMnuUl7t9qEyWZM12VjFhEz6LMoVMc7tVAHs0KYKLAVd6B9bU6uwY1hKJdX
SNXmKZV1heEaa8t5niSBpIgGwS05+uFGz/POrMUL6ZaQGHBi3lm0+i03QryNQTpO
ecPXgtUYxVcf4m/2GLIB8gRVyFkqc93fnmThZ0uT8HZFGo9M5PykxyBSFxOxqHmg
BrQ5Ci5nz2TodBzF00bJTxfJ/5bONjhj4gfZweX+hneJqbVOpC8ZofCk7H0k1sYq
porF9ruZDhy+Lfuy3fWObyQB3wDekDFoZiNxEs2T6fYuvjLixjoY5pxldkotIv7m
kur71ET7AgMBAAECggEABgy78Hzlzz+X0Hv7V5ekwGDThKpwVgbgqaFaGjdP1W8F
ep9httfALipX7MqFx8K1xLjXtX/Q4bSMdwPrjcS9nNqXqerPWfykRXu2qh9WvLpC
W+bVQdUxQxRGlbU+s5YPAGd5ANios7eJsptngWDEpifzA+L7bfP3vO+2yeaDzIB3
JQ+wYST0SWm3x3FGM7SCwA9wpNBA38igTJmlppdZIoifzTsb/Q34NmWUEqijVTB8
DbofDAVW2c9qW/78VABLvDGSOcQUqclDPInBvVjQ8nzv1BaagkyXeKIwRSR3t6Py
WVJge9fEirgv8nnqKbpLqfPrYTJPit3revUHeheHgQKBgQD9yDujq4Ty3VQq3ajd
/GoV0NCaABaCeKfmIiiFo7NJRpCS8lrxMvDRfnsl6fCBFPfK8YU4HUxh6s0Rmxx6
UvDlD/7w3DFeUj/h+2/N3Nj+rYCFTJNens7lvvZS2ftt56d0DBk1JLTxQh+N/OX5
t82ZMRS/R4f/ibuWFvyJAAGFQQKBgQDNjsodPy6aXVCR82NxEJ3XLuZWCwJoFC7s
XMMWpjmXCLduBxVIJdCd21L74zX892o1uBLwsuQZZPVUd0zCmCXhonxkhpit3I4S
qAs86zxmZE9QsoBDk6ECDZ6t5OiBMA6AwdMH4e8GOkZDLcizfc2CINipb8+U0qqC
tBCwmvvPOwKBgCM2FfhGgwLDbLsp2BU8wWdXeqnzWywtG3aVxLOOHAENtl99Gtse
a0VV3DZNeB4gz6Sr0AUSI5fuYReRQulB+sR9bKz0kDD7DnwHS+LvQnhLkGpuToAx
XpmH3ltufTEplBVI3HKALk7PEtu7fBkixHb91VgYz6jH7mwLsmw7wPpBAoGAEvNp
Gs0qZLzZorsHnfLkOmRug9w7+pBxywS6T6o/gPciwhgRFDe4RfVkbyiBX7MHrbAs
vtgfQ2AVZhYhk4cnZufuA+6MwOqmhn3Lm3Asf1wcG9p5DMHdhCzxRiLmdJKTo7c6
120y9iYFOEhOSo38llSk5OoT/yp04dvr9fwz3uUCgYEA59f1MW3cQ/p1+w/k44e+
s16ZGDxNc06rhp4oTpM+Ey5RtGgkWh0R10EbeXQEEpmfy4tf3OoKb9UCgSLYvGxP
jIPhvqZJe3pGRdwJG55rJLtS466z5MKG/WKmqFecLejDcVg9qblh9AW5PSvyzcQW
gT7yGRIIu9uNETw/d7mV+7Y=
-----END PRIVATE KEY-----''';

// ─── Helpers ──────────────────────────────────────────────────────

String fmtSize(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
  return '${(bytes / 1024 / 1024).toStringAsFixed(2)} MB';
}

/// Navigator key — lets the demo picker open its sheet from anywhere.
final demoNavigatorKey = GlobalKey<NavigatorState>();

// ─── Demo picker — sample PDFs generated in memory ────────────────
//
// No file system, no platform picker, no permissions: every sample is
// produced on demand by the package's own builder (so the picker
// doubles as a live builder showcase) and works identically on every
// platform including web.

class _DemoSample {
  const _DemoSample(this.name, this.icon, this.build);
  final String name;
  final IconData icon;
  final Future<Uint8List> Function(Pdf pdf) build;
}

final List<_DemoSample> _demoSamples = [
  _DemoSample(
      'Tiny blank page', Icons.crop_portrait, (pdf) async => minimalPdf),
  _DemoSample('Text report (5 pages)', Icons.article, (pdf) async {
    final b = await pdf.build();
    await b.setTitle('Demo Report');
    for (var i = 1; i <= 5; i++) {
      final p = await b.addA4Page();
      await p.heading(2, 'Section $i');
      await p.space(8);
      await p
          .paragraph('Demo paragraph for section $i. The quick brown fox jumps '
              'over the lazy dog while the engine streams every byte.');
      await p.done();
    }
    final sink = DemoSink();
    await b.save(sink);
    await b.dispose();
    return sink.takeBytes();
  }),
  _DemoSample('Long document (40 pages)', Icons.menu_book,
      (pdf) => buildSamplePdf(pdf)),
  _DemoSample('Form sample', Icons.fact_check, (pdf) async {
    final b = await pdf.build();
    await b.setTitle('Demo Form');
    final p = await b.addA4Page();
    await p.heading(1, 'Application Form');
    await p.space(10);
    await p.text('Name:');
    await p.textField(
        'name', const PdfRect(x: 100, y: 680, width: 200, height: 20));
    await p.text('Agree:');
    await p.checkbox(
        'agree', const PdfRect(x: 100, y: 640, width: 14, height: 14));
    await p.done();
    final sink = DemoSink();
    await b.save(sink);
    await b.dispose();
    return sink.takeBytes();
  }),
];

/// Demo replacement for a platform file picker: a sheet of sample
/// PDFs, generated in memory on selection. Same signature as before —
/// call sites are untouched.
Future<List<Uint8List>?> pickPdfBytes({bool multiple = false}) async {
  final context = demoNavigatorKey.currentContext;
  if (context == null) return null;
  final picked = await showModalBottomSheet<List<_DemoSample>>(
    context: context,
    showDragHandle: true,
    builder: (sheetCtx) => _DemoSampleSheet(multiple: multiple),
  );
  if (picked == null || picked.isEmpty) return null;
  final pdf = Pdf();
  try {
    return [for (final s in picked) await s.build(pdf)];
  } finally {
    await pdf.dispose();
  }
}

class _DemoSampleSheet extends StatefulWidget {
  const _DemoSampleSheet({required this.multiple});
  final bool multiple;
  @override
  State<_DemoSampleSheet> createState() => _DemoSampleSheetState();
}

class _DemoSampleSheetState extends State<_DemoSampleSheet> {
  final _selected = <_DemoSample>{};

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: Column(mainAxisSize: MainAxisSize.min, children: [
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
          child: Row(children: [
            Icon(Icons.auto_awesome,
                color: Theme.of(context).colorScheme.primary),
            const SizedBox(width: 8),
            Text(
                widget.multiple
                    ? 'Pick demo PDFs (built in memory)'
                    : 'Pick a demo PDF (built in memory)',
                style: Theme.of(context).textTheme.titleMedium),
          ]),
        ),
        for (final sample in _demoSamples)
          widget.multiple
              ? CheckboxListTile(
                  secondary: Icon(sample.icon),
                  title: Text(sample.name),
                  value: _selected.contains(sample),
                  onChanged: (v) => setState(() => v == true
                      ? _selected.add(sample)
                      : _selected.remove(sample)),
                )
              : ListTile(
                  leading: Icon(sample.icon),
                  title: Text(sample.name),
                  onTap: () => Navigator.pop(context, [sample]),
                ),
        if (widget.multiple)
          Padding(
            padding: const EdgeInsets.all(12),
            child: FilledButton.icon(
              key: const ValueKey('demo-picker-use'),
              onPressed: _selected.isEmpty
                  ? null
                  : () => Navigator.pop(context, _selected.toList()),
              icon: const Icon(Icons.check),
              label: Text('Use ${_selected.length} files'),
            ),
          ),
      ]),
    );
  }
}

/// Tiny valid 1×1 PNG — the demo stand-in for picked images.
final Uint8List demoPng = Uint8List.fromList(const [
  0x89,
  0x50,
  0x4E,
  0x47,
  0x0D,
  0x0A,
  0x1A,
  0x0A,
  0x00,
  0x00,
  0x00,
  0x0D,
  0x49,
  0x48,
  0x44,
  0x52,
  0x00,
  0x00,
  0x00,
  0x01,
  0x00,
  0x00,
  0x00,
  0x01,
  0x08,
  0x02,
  0x00,
  0x00,
  0x00,
  0x90,
  0x77,
  0x53,
  0xDE,
  0x00,
  0x00,
  0x00,
  0x0C,
  0x49,
  0x44,
  0x41,
  0x54,
  0x08,
  0xD7,
  0x63,
  0xF8,
  0xCF,
  0xC0,
  0x00,
  0x00,
  0x00,
  0x03,
  0x00,
  0x01,
  0xCE,
  0xCC,
  0x09,
  0x4B,
  0x00,
  0x00,
  0x00,
  0x00,
  0x49,
  0x45,
  0x4E,
  0x44,
  0xAE,
  0x42,
  0x60,
  0x82,
]);

/// Demo replacement for image picking — returns the bundled sample.
Future<List<({String name, Uint8List bytes, int size})>?>
    pickImageBytes() async =>
        [(name: 'demo.png', bytes: demoPng, size: demoPng.length)];

/// Demo replacement for save dialogs: outputs stay in memory; report
/// the size instead of touching the file system.
Future<String?> saveBytes(Uint8List bytes, String name) async {
  final context = demoNavigatorKey.currentContext;
  if (context != null) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(
        content:
            Text('$name ready — ${fmtSize(bytes.length)} (kept in memory)')));
  }
  return name;
}

/// Builds a [pages]-page text PDF with the given instance — gives the
/// heavy demos something real to chew on without picking a file.
Future<Uint8List> buildSamplePdf(Pdf pdf, {int pages = 40}) async {
  final b = await pdf.build();
  try {
    await b.setTitle('Sample ($pages pages)');
    for (var i = 0; i < pages; i++) {
      final p = await b.addA4Page();
      await p.heading(2, 'Page ${i + 1} of $pages');
      await p.space(10);
      for (var j = 0; j < 6; j++) {
        await p.paragraph(
            'Lorem ipsum dolor sit amet, consectetur adipiscing elit. '
            'Sed do eiusmod tempor incididunt ut labore et dolore magna '
            'aliqua. Ut enim ad minim veniam, quis nostrud exercitation.');
        await p.space(6);
      }
      await p.done();
    }
    final sink = DemoSink();
    await b.save(sink);
    return sink.takeBytes();
  } finally {
    await b.dispose();
  }
}

// ─── App shell ────────────────────────────────────────────────────

/// The bundled Noto Emoji face. Tries the standalone asset key first,
/// then the package-prefixed key the example_trimmed shell serves it
/// under (assets of a dependency package are keyed `packages/<name>/…`).
Future<ByteData> loadEmojiFont() async {
  try {
    return await rootBundle.load('assets/fonts/NotoEmoji-Regular.ttf');
  } on FlutterError {
    return rootBundle.load(
        'packages/pdf_manipulator_example/assets/fonts/NotoEmoji-Regular.ttf');
  }
}

void main() => runApp(const ExampleApp());

class ExampleApp extends StatelessWidget {
  const ExampleApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      navigatorKey: demoNavigatorKey,
      title: 'PDF Manipulator',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(useMaterial3: true, colorSchemeSeed: Colors.deepPurple),
      darkTheme: ThemeData(
          useMaterial3: true,
          brightness: Brightness.dark,
          colorSchemeSeed: Colors.deepPurple),
      themeMode: ThemeMode.system,
      home: const HomePage(),
    );
  }
}

class HomePage extends StatelessWidget {
  const HomePage({super.key});

  @override
  Widget build(BuildContext context) {
    return DefaultTabController(
      length: 7,
      child: Scaffold(
        appBar: AppBar(
          title: const Text('PDF Manipulator'),
          actions: const [_IoModeChip(), SizedBox(width: 12)],
          bottom: const TabBar(
            isScrollable: true,
            tabAlignment: TabAlignment.start,
            tabs: [
              Tab(icon: Icon(Icons.bolt, size: 20), text: 'Runtime'),
              Tab(icon: Icon(Icons.description, size: 20), text: 'Doc'),
              Tab(icon: Icon(Icons.auto_fix_high, size: 20), text: 'Sugar'),
              Tab(icon: Icon(Icons.output, size: 20), text: 'Standalone'),
              Tab(icon: Icon(Icons.edit_document, size: 20), text: 'Editor'),
              Tab(icon: Icon(Icons.note_add, size: 20), text: 'Builder'),
              Tab(icon: Icon(Icons.merge, size: 20), text: 'Merge'),
            ],
          ),
        ),
        body: const TabBarView(children: [
          _RuntimeTab(),
          _DocTab(),
          _SugarTab(),
          _StandaloneTab(),
          _EditorTab(),
          _BuilderTab(),
          _MergeTab(),
        ]),
      ),
    );
  }
}

/// Live chip showing the detected I/O mode (native / jspi / atomics /
/// opfs). Its own tiny Pdf instance — instances are cheap and fully
/// independent.
class _IoModeChip extends StatefulWidget {
  const _IoModeChip();
  @override
  State<_IoModeChip> createState() => _IoModeChipState();
}

class _IoModeChipState extends State<_IoModeChip> {
  final _probe = Pdf();
  PdfIoMode? _mode;

  @override
  void initState() {
    super.initState();
    _probe.ensureInitialized().then((m) {
      if (mounted) setState(() => _mode = m);
    });
  }

  @override
  void dispose() {
    unawaited(_probe.dispose());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    return Center(
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
        decoration: BoxDecoration(
          color: cs.primaryContainer,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Text(
          _mode?.name.toUpperCase() ?? '…',
          style: TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w700,
              letterSpacing: 0.5,
              color: cs.onPrimaryContainer),
        ),
      ),
    );
  }
}

// ─── Shared widgets ───────────────────────────────────────────────

class _Status extends StatelessWidget {
  final bool loading;
  final String? message;
  final VoidCallback? onDismiss;
  final VoidCallback? onCancel;
  const _Status(
      {this.loading = false, this.message, this.onDismiss, this.onCancel});

  @override
  Widget build(BuildContext context) {
    if (!loading && message == null) return const SizedBox.shrink();
    final cs = Theme.of(context).colorScheme;
    final isErr = message?.startsWith('Error') == true;
    final bg = loading
        ? cs.primaryContainer
        : (isErr ? cs.errorContainer : cs.tertiaryContainer);
    final fg = loading
        ? cs.onPrimaryContainer
        : (isErr ? cs.onErrorContainer : cs.onTertiaryContainer);
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      color: bg,
      child: Row(children: [
        if (loading) ...[
          SizedBox(
              width: 14,
              height: 14,
              child: CircularProgressIndicator(strokeWidth: 2, color: fg)),
          const SizedBox(width: 12),
        ],
        Expanded(
            child: Text(message ?? '',
                key: const ValueKey('status-text'),
                style: TextStyle(color: fg, fontSize: 13),
                maxLines: 4)),
        if (loading && onCancel != null)
          TextButton.icon(
            onPressed: onCancel,
            icon: Icon(Icons.cancel, size: 16, color: fg),
            label: Text('Cancel', style: TextStyle(color: fg, fontSize: 12)),
          ),
        if (!loading && onDismiss != null)
          GestureDetector(
              onTap: onDismiss, child: Icon(Icons.close, size: 16, color: fg)),
      ]),
    );
  }
}

class _Section extends StatelessWidget {
  final String text;
  const _Section(this.text);
  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(top: 14, bottom: 4, left: 4),
        child: Text(text.toUpperCase(),
            style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w700,
                letterSpacing: 1.2,
                color: Theme.of(context).colorScheme.primary)),
      );
}

class _Op extends StatelessWidget {
  final IconData icon;
  final String title;
  final String? subtitle;
  final VoidCallback onRun;
  final bool loading;
  const _Op(
      {required this.icon,
      required this.title,
      this.subtitle,
      required this.onRun,
      this.loading = false});

  @override
  Widget build(BuildContext context) => Card(
        margin: const EdgeInsets.symmetric(vertical: 2),
        child: ListTile(
          leading: Icon(icon, size: 22),
          title: Text(title,
              style:
                  const TextStyle(fontSize: 14, fontWeight: FontWeight.w500)),
          subtitle: subtitle != null
              ? Text(subtitle!, style: const TextStyle(fontSize: 12))
              : null,
          trailing: SizedBox(
              width: 64,
              height: 34,
              child: FilledButton.tonal(
                // Stable handle for integration tests ("run:<title>").
                key: ValueKey('run:$title'),
                onPressed: loading ? null : onRun,
                style: FilledButton.styleFrom(
                    padding: EdgeInsets.zero,
                    textStyle: const TextStyle(fontSize: 13)),
                child: const Text('Run'),
              )),
          dense: true,
          visualDensity: VisualDensity.compact,
        ),
      );
}

class _EmptyState extends StatelessWidget {
  final IconData icon;
  final String text;
  final VoidCallback? onPick;
  final String pickLabel;
  const _EmptyState(
      {required this.icon,
      required this.text,
      this.onPick,
      this.pickLabel = 'Open PDF'});

  @override
  Widget build(BuildContext context) => Center(
        child: Column(mainAxisSize: MainAxisSize.min, children: [
          Icon(icon,
              size: 56,
              color:
                  Theme.of(context).colorScheme.primary.withValues(alpha: 0.5)),
          const SizedBox(height: 16),
          Text(text),
          if (onPick != null) ...[
            const SizedBox(height: 16),
            FilledButton.icon(
                onPressed: onPick,
                icon: const Icon(Icons.file_open),
                label: Text(pickLabel)),
          ],
        ]),
      );
}

void showTextSheet(BuildContext context, String title, String text) {
  showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    builder: (ctx) => DraggableScrollableSheet(
      expand: false,
      initialChildSize: 0.7,
      maxChildSize: 0.95,
      builder: (ctx, sc) => Column(children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 16, 8, 8),
          child: Row(children: [
            Expanded(
                child: Text(title,
                    style: const TextStyle(
                        fontWeight: FontWeight.w600, fontSize: 16))),
            IconButton(
                icon: const Icon(Icons.close),
                onPressed: () => Navigator.pop(ctx)),
          ]),
        ),
        const Divider(height: 1),
        Expanded(
          child: SingleChildScrollView(
            controller: sc,
            padding: const EdgeInsets.all(16),
            child: SelectableText(text.isEmpty ? '(empty)' : text,
                style: const TextStyle(
                    fontSize: 13, fontFamily: 'monospace', height: 1.5)),
          ),
        ),
      ]),
    ),
  );
}

// ─── The op runner — cancellation-aware, shared by every tab ──────

/// Per-tab op execution: loading state, status line, and a live
/// Cancel button whenever the running op is a single [PdfTask].
mixin _OpsRunner<T extends StatefulWidget> on State<T> {
  bool loading = false;
  String? status;
  PdfTask<Object?>? _task;

  /// Runs one cancellable engine task. Returns true on success,
  /// false on cancellation or error (already reported in [status]).
  Future<bool> runTask<R>(String label, PdfTask<R> Function() start,
      {String Function(R result)? done}) async {
    final task = start();
    setState(() {
      loading = true;
      status = '$label…';
      _task = task;
    });
    try {
      final result = await task;
      setState(() => status = done?.call(result) ?? '$label done');
      return true;
    } on PdfCancelled {
      setState(() => status =
          'Cancelled — the instance and every other handle keep working');
      return false;
    } catch (e) {
      setState(() => status = 'Error: $e');
      return false;
    } finally {
      if (mounted) {
        setState(() {
          loading = false;
          _task = null;
        });
      }
    }
  }

  /// Runs a multi-step flow (not cancellable as a unit). Returns
  /// true on success.
  Future<bool> runFlow(String label, Future<void> Function() body) async {
    setState(() {
      loading = true;
      status = '$label…';
    });
    try {
      await body();
      return true;
    } on PdfCancelled {
      setState(() => status = 'Cancelled');
      return false;
    } catch (e) {
      setState(() => status = 'Error: $e');
      return false;
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  Future<void> saveResult(Uint8List bytes, String name) async {
    final p = await saveBytes(bytes, name);
    setState(() => status = p != null
        ? 'Saved $name (${fmtSize(bytes.length)})'
        : 'Save cancelled');
  }

  Widget statusBar() => _Status(
        loading: loading,
        message: status,
        onCancel: _task == null ? null : () => _task?.cancel(),
        onDismiss: () => setState(() => status = null),
      );
}

/// Shared "pick a PDF" state for tabs that operate on one input file.
mixin _PdfPicker<T extends StatefulWidget> on State<T> {
  Uint8List? bytes;

  DataSource get src => DemoSource(bytes!);

  Future<void> pick(void Function(Uint8List picked) onPicked) async {
    final r = await pickPdfBytes();
    if (r == null || r.isEmpty) return;
    setState(() => bytes = r.first);
    onPicked(r.first);
  }

  Widget fileCard(BuildContext context, VoidCallback onSwap) => Card(
        child: ListTile(
          leading: Icon(Icons.picture_as_pdf,
              color: Theme.of(context).colorScheme.primary),
          title: Text(fmtSize(bytes!.length)),
          trailing: IconButton.filledTonal(
              onPressed: onSwap, icon: const Icon(Icons.swap_horiz, size: 20)),
        ),
      );
}

// ─── Tab 1: Runtime — the lane architecture, live ─────────────────

class _RuntimeTab extends StatefulWidget {
  const _RuntimeTab();
  @override
  State<_RuntimeTab> createState() => _RuntimeTabState();
}

class _RuntimeTabState extends State<_RuntimeTab>
    with _OpsRunner, AutomaticKeepAliveClientMixin {
  final _pdf = Pdf();
  Uint8List? _sample;

  @override
  bool get wantKeepAlive => true;

  @override
  void dispose() {
    unawaited(_pdf.dispose());
    super.dispose();
  }

  /// 40-page sample, built once, reused by the heavy demos.
  Future<Uint8List> _ensureSample() async =>
      _sample ??= await buildSamplePdf(_pdf);

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Column(children: [
      statusBar(),
      Expanded(
        child: ListView(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          children: [
            Card(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Row(children: [
                  Icon(Icons.bolt,
                      color: Theme.of(context).colorScheme.primary, size: 28),
                  const SizedBox(width: 12),
                  const Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text('The lane architecture',
                              style: TextStyle(
                                  fontWeight: FontWeight.w600, fontSize: 15)),
                          Text(
                              'Every op runs on an isolated lane. Cancel one '
                              'op, or kill the whole instance instantly — '
                              'no demo here needs a file.',
                              style: TextStyle(fontSize: 12)),
                        ]),
                  ),
                ]),
              ),
            ),
            _Section('Per-op cancellation (PdfTask)'),
            _Op(
                icon: Icons.cancel_schedule_send,
                title: 'Heavy merge — hit Cancel while it runs',
                subtitle: 'merge of 6 × 40-page docs; Cancel appears above',
                loading: loading,
                onRun: () async {
                  final sample = await _ensureSample();
                  await runTask(
                      'Heavy merge (tap Cancel!)',
                      () => _pdf.merge(
                          List.filled(6, DemoSource(sample)), DemoSink()),
                      done: (_) => 'Merge finished before you cancelled');
                }),
            _Op(
                icon: Icons.timer_off,
                title: 'Cancel before the job even starts',
                subtitle: 'task.cancel() on the same tick → PdfCancelled',
                loading: loading,
                onRun: () => runFlow('Cancel-before-start', () async {
                      final task = _pdf.open(DemoSource(minimalPdf));
                      task.cancel();
                      try {
                        final doc = await task;
                        await doc.dispose();
                        setState(() =>
                            status = 'Completed before the cancel landed');
                      } on PdfCancelled {
                        setState(() => status =
                            '✓ PdfCancelled — the job never reached a lane');
                      }
                    })),
            _Op(
                icon: Icons.healing,
                title: 'Cancelled op leaves siblings untouched',
                subtitle: 'cancel an extract; the open doc keeps working',
                loading: loading,
                onRun: () => runFlow('Sibling survival', () async {
                      final sample = await _ensureSample();
                      final doc = await _pdf.open(DemoSource(sample));
                      try {
                        final task = doc.extract(pages: const PdfPages.all());
                        task.cancel();
                        try {
                          await task;
                        } on PdfCancelled {
                          // expected — now prove the doc still works:
                        }
                        final page0 =
                            await doc.extract(pages: const PdfPages.single(0));
                        setState(() =>
                            status = '✓ extract cancelled; same doc then read '
                                '${page0.length} chars from page 1');
                      } finally {
                        await doc.dispose();
                      }
                    })),
            _Section('Parallel ops'),
            _Op(
                icon: Icons.call_split,
                title: 'Open 4 documents in parallel',
                subtitle: 'Future.wait — each lands on its own lane',
                loading: loading,
                onRun: () => runFlow('Parallel opens', () async {
                      final sw = Stopwatch()..start();
                      final docs = await Future.wait(List.generate(
                          4, (_) => _pdf.open(DemoSource(minimalPdf))));
                      sw.stop();
                      final pages = docs.map((d) => d.pageCount).join(', ');
                      for (final d in docs) {
                        await d.dispose();
                      }
                      setState(() =>
                          status = '4 docs open in ${sw.elapsedMilliseconds}ms '
                              '(pages: $pages)');
                    })),
            _Section('Instant dispose'),
            _Op(
                icon: Icons.power_settings_new,
                title: 'Dispose mid-flight — measure it',
                subtitle:
                    'starts a heavy merge on a scratch instance, then kills it',
                loading: loading,
                onRun: () => runFlow('Instant dispose', () async {
                      final sample = await _ensureSample();
                      final lab = Pdf();
                      final inflight = lab.merge(
                          List.filled(6, DemoSource(sample)), DemoSink());
                      await Future<void>.delayed(
                          const Duration(milliseconds: 30));
                      final sw = Stopwatch()..start();
                      await lab.dispose();
                      sw.stop();
                      var outcome = 'op finished first';
                      try {
                        await inflight;
                      } on PdfCancelled {
                        outcome = 'in-flight op resolved as PdfCancelled';
                      }
                      setState(() => status =
                          '✓ dispose() returned in ${sw.elapsedMicroseconds / 1000}ms; '
                              '$outcome');
                    })),
            _Op(
                icon: Icons.fiber_new,
                title: 'Fresh instance works after an abrupt kill',
                loading: loading,
                onRun: () => runFlow('Fresh after kill', () async {
                      final lab = Pdf();
                      unawaited(lab.open(DemoSource(minimalPdf)));
                      await lab.dispose(); // killed mid-open — fine
                      final lab2 = Pdf();
                      try {
                        final doc = await lab2.open(DemoSource(minimalPdf));
                        setState(() => status =
                            '✓ new instance opened ${doc.pageCount} page(s) '
                                'right after the kill');
                        await doc.dispose();
                      } finally {
                        await lab2.dispose();
                      }
                    })),
            const SizedBox(height: 40),
          ],
        ),
      ),
    ]);
  }
}

// ─── Tab 2: PdfDoc — read-only queries ────────────────────────────

class _DocTab extends StatefulWidget {
  const _DocTab();
  @override
  State<_DocTab> createState() => _DocTabState();
}

class _DocTabState extends State<_DocTab>
    with _OpsRunner, _PdfPicker, AutomaticKeepAliveClientMixin {
  final _pdf = Pdf();
  PdfDoc? _doc;

  @override
  bool get wantKeepAlive => true;

  @override
  void dispose() {
    unawaited(_pdf.dispose());
    super.dispose();
  }

  Future<void> _open() => pick((picked) {
        unawaited(runFlow('Opening', () async {
          final old = _doc;
          _doc = null;
          if (old != null) await old.dispose();
          final doc = await _pdf.open(DemoSource(picked));
          setState(() {
            _doc = doc;
            status = '${doc.pageCount} pages, v${doc.version}';
          });
        }));
      });

  @override
  Widget build(BuildContext context) {
    super.build(context);
    final doc = _doc;
    return Column(children: [
      statusBar(),
      Expanded(
        child: doc == null
            ? _EmptyState(
                icon: Icons.description,
                text: 'Open a PDF to query it',
                onPick: loading ? null : _open)
            : ListView(
                padding:
                    const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                children: [
                  Card(
                    child: Padding(
                      padding: const EdgeInsets.all(16),
                      child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Row(children: [
                              Icon(Icons.picture_as_pdf,
                                  color: Theme.of(context).colorScheme.primary,
                                  size: 28),
                              const SizedBox(width: 12),
                              Expanded(
                                child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    children: [
                                      Text(
                                          '${doc.pageCount} pages  •  '
                                          '${fmtSize(bytes!.length)}  •  '
                                          'v${doc.version}',
                                          style: const TextStyle(fontSize: 13)),
                                      if (doc.title != null)
                                        Text('Title: ${doc.title}',
                                            style:
                                                const TextStyle(fontSize: 12)),
                                      if (doc.author != null)
                                        Text('Author: ${doc.author}',
                                            style:
                                                const TextStyle(fontSize: 12)),
                                    ]),
                              ),
                              IconButton.filledTonal(
                                  onPressed: _open,
                                  icon: const Icon(Icons.swap_horiz, size: 20)),
                            ]),
                            const SizedBox(height: 8),
                            Wrap(spacing: 6, runSpacing: 4, children: [
                              if (doc.isEncrypted)
                                _chip(context, 'Encrypted', warn: true),
                              if (doc.isTagged) _chip(context, 'Tagged'),
                              if (doc.subject != null)
                                _chip(context, doc.subject!),
                              if (doc.keywords != null)
                                _chip(context, doc.keywords!),
                            ]),
                          ]),
                    ),
                  ),
                  _Section('Page Dimensions'),
                  ...List.generate(
                      doc.pageCount.clamp(0, 20),
                      (i) => Padding(
                            padding: const EdgeInsets.symmetric(
                                vertical: 2, horizontal: 12),
                            child: Text(
                                'Page ${i + 1}: '
                                '${doc.pages[i].effectiveWidth.toStringAsFixed(0)} × '
                                '${doc.pages[i].effectiveHeight.toStringAsFixed(0)} pt'
                                '${doc.pages[i].rotation != 0 ? '  (${doc.pages[i].rotation}°)' : ''}',
                                style: const TextStyle(fontSize: 13)),
                          )),
                  if (doc.pageCount > 20)
                    Text('+ ${doc.pageCount - 20} more',
                        style: const TextStyle(fontSize: 12)),
                  _Section('Text Extraction'),
                  _Op(
                      icon: Icons.text_snippet,
                      title: 'Extract all (plain text)',
                      loading: loading,
                      onRun: () => runTask('Extract',
                              () => doc.extract(pages: const PdfPages.all()),
                              done: (t) {
                            showTextSheet(
                                context, 'Plain Text (${t.length} chars)', t);
                            return '${t.length} chars';
                          })),
                  _Op(
                      icon: Icons.text_snippet_outlined,
                      title: 'Extract page 1 only',
                      loading: loading,
                      onRun: () => runTask(
                              'Extract p1',
                              () =>
                                  doc.extract(pages: const PdfPages.single(0)),
                              done: (t) {
                            showTextSheet(context, 'Page 1', t);
                            return 'Page 1: ${t.length} chars';
                          })),
                  _Op(
                      icon: Icons.code,
                      title: 'Extract as Markdown',
                      loading: loading,
                      onRun: () => runTask(
                              'Markdown',
                              () => doc.extract(
                                  pages: const PdfPages.all(),
                                  format: PdfExtractionFormat.markdown),
                              done: (t) {
                            showTextSheet(context, 'Markdown', t);
                            return 'MD: ${t.length} chars';
                          })),
                  _Op(
                      icon: Icons.html,
                      title: 'Extract as HTML (page 1)',
                      loading: loading,
                      onRun: () => runTask(
                              'HTML',
                              () => doc.extract(
                                  pages: const PdfPages.single(0),
                                  format: PdfExtractionFormat.html), done: (t) {
                            showTextSheet(context, 'HTML', t);
                            return 'HTML: ${t.length} chars';
                          })),
                  _Section('Search'),
                  _Op(
                      icon: Icons.search,
                      title: 'Search "the" on page 1',
                      loading: loading,
                      onRun: () => runTask(
                          'Search p1',
                          () => doc.search(
                              query: 'the', pages: const PdfPages.single(0)),
                          done: (r) => '${r.length} hits on page 1')),
                  _Op(
                      icon: Icons.manage_search,
                      title: 'Search "the" all pages',
                      loading: loading,
                      onRun: () => runTask(
                          'Search all',
                          () => doc.search(
                              query: 'the', pages: const PdfPages.all()),
                          done: (r) => '${r.length} hits total')),
                  _Section('Render & Images'),
                  _Op(
                      icon: Icons.image,
                      title: 'Render page 1 to image',
                      loading: loading,
                      onRun: () => runFlow('Render', () async {
                            var info = 'No pages rendered';
                            await for (final page in doc.render(
                                pages: const PdfPages.single(0))) {
                              info = 'Rendered ${page.width}×${page.height} '
                                  '(${fmtSize(page.data.length)})';
                            }
                            setState(() => status = info);
                          })),
                  _Op(
                      icon: Icons.photo_library,
                      title: 'Extract images from page 1',
                      loading: loading,
                      onRun: () => runFlow('Images', () async {
                            var count = 0;
                            await for (final _ in doc.extractImages(
                                pages: const PdfPages.single(0))) {
                              count++;
                            }
                            setState(
                                () => status = '$count image(s) on page 1');
                          })),
                  _Section('Signatures'),
                  _Op(
                      icon: Icons.draw,
                      title: 'List signatures',
                      loading: loading,
                      onRun: () => runTask(
                          'Signatures', () => doc.getSignatures(),
                          done: (sigs) => '${sigs.length} signature(s)')),
                  _Op(
                      icon: Icons.verified_user,
                      title: 'Verify signatures',
                      loading: loading,
                      onRun: () => runTask(
                          'Verify', () => doc.verifySignatures(),
                          done: (ok) => ok ? 'Valid ✓' : 'Invalid or none ✗')),
                  _Section('Validation'),
                  _Op(
                      icon: Icons.verified,
                      title: 'Validate PDF/A',
                      loading: loading,
                      onRun: () => runTask('PDF/A', () => doc.validatePdfA(),
                          done: (r) =>
                              '${r.compliant ? "Compliant ✓" : "Not compliant"} '
                              '(${r.errors}e ${r.warnings}w)')),
                  _Op(
                      icon: Icons.accessibility,
                      title: 'Validate PDF/UA',
                      loading: loading,
                      onRun: () => runTask('PDF/UA', () => doc.validatePdfUa(),
                          done: (r) =>
                              r ? 'Accessible ✓' : 'Not accessible ✗')),
                  _Section('Classification'),
                  _Op(
                      icon: Icons.category,
                      title: 'Classify page 1',
                      loading: loading,
                      onRun: () => runTask(
                          'Classify page', () => doc.classifyPage(0),
                          done: (r) => 'Page 1: ${r.type}')),
                  _Op(
                      icon: Icons.analytics,
                      title: 'Classify document',
                      loading: loading,
                      onRun: () => runTask(
                          'Classify doc', () => doc.classifyDocument(),
                          done: (r) => 'Document: ${r.type}')),
                  _Section('Bookmarks'),
                  _Op(
                      icon: Icons.bookmark,
                      title: 'Plan split by bookmarks',
                      loading: loading,
                      onRun: () => runFlow('Bookmarks', () async {
                            try {
                              final s = await doc.planSplitByBookmarks();
                              setState(() => status = '${s.length} segments: '
                                  '${s.map((x) => x.title).join(', ')}');
                            } catch (_) {
                              setState(() => status = 'No bookmarks');
                            }
                          })),
                  const SizedBox(height: 40),
                ],
              ),
      ),
    ]);
  }

  Widget _chip(BuildContext context, String text, {bool warn = false}) {
    final cs = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
          color: warn ? cs.errorContainer : cs.surfaceContainerHighest,
          borderRadius: BorderRadius.circular(12)),
      child: Text(text,
          style: TextStyle(
              fontSize: 11,
              color: warn ? cs.onErrorContainer : cs.onSurfaceVariant)),
    );
  }
}

// ─── Tab 3: PdfSugar — one-shot convenience ops ───────────────────

class _SugarTab extends StatefulWidget {
  const _SugarTab();
  @override
  State<_SugarTab> createState() => _SugarTabState();
}

class _SugarTabState extends State<_SugarTab>
    with _OpsRunner, _PdfPicker, AutomaticKeepAliveClientMixin {
  final _pdf = Pdf();

  @override
  bool get wantKeepAlive => true;

  @override
  void dispose() {
    unawaited(_pdf.dispose());
    super.dispose();
  }

  void _pickFile() => unawaited(
      pick((p) => setState(() => status = 'Loaded (${fmtSize(p.length)})')));

  /// One-shot op into a sink, then offer to save the result.
  ///
  /// This tab uses the shipped MemorySink (from the package) rather than
  /// the example's own DemoSink — the quick path, no plumbing to write.
  Future<void> _opToFile(String label, String filename,
      PdfTask<void> Function(MemorySink sink) start) async {
    final sink = MemorySink();
    if (await runTask(label, () => start(sink))) {
      await saveResult(sink.takeBytes(), filename);
    }
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Column(children: [
      statusBar(),
      Expanded(
        child: bytes == null
            ? _EmptyState(
                icon: Icons.auto_fix_high,
                text: 'Pick a PDF for one-shot ops',
                onPick: _pickFile)
            : ListView(
                padding:
                    const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                children: [
                  fileCard(context, _pickFile),
                  _Section('Structural'),
                  _Op(
                      icon: Icons.content_cut,
                      title: 'Split every 2 pages',
                      loading: loading,
                      onRun: () => runFlow('Split', () async {
                            final sinks = <DemoSink>[];
                            await _pdf.split(src, (i) {
                              final s = DemoSink();
                              sinks.add(s);
                              return s;
                            }, every: 2);
                            setState(() => status = '${sinks.length} chunks: '
                                '${sinks.map((s) => fmtSize(s.length)).join(', ')}');
                          })),
                  _Op(
                      icon: Icons.straighten,
                      title: 'Split by size (500 KB)',
                      loading: loading,
                      onRun: () => runFlow('SplitBySize', () async {
                            final sinks = <DemoSink>[];
                            await _pdf.splitBySize(src, (i) {
                              final s = DemoSink();
                              sinks.add(s);
                              return s;
                            }, maxBytes: 500000);
                            setState(() => status = '${sinks.length} chunks: '
                                '${sinks.map((s) => fmtSize(s.length)).join(', ')}');
                          })),
                  _Op(
                      icon: Icons.call_split,
                      title: 'Split by bookmarks',
                      loading: loading,
                      onRun: () => runFlow('SplitBookmarks', () async {
                            try {
                              final sinks = <DemoSink>[];
                              await _pdf.splitByBookmarks(src, (i) {
                                final s = DemoSink();
                                sinks.add(s);
                                return s;
                              });
                              setState(() => status = '${sinks.length} parts');
                            } catch (_) {
                              setState(() => status = 'No bookmarks');
                            }
                          })),
                  _Op(
                      icon: Icons.file_copy_outlined,
                      title: 'Extract pages 0–1',
                      loading: loading,
                      onRun: () => _opToFile(
                          'Extract',
                          'extracted.pdf',
                          (sink) =>
                              _pdf.extractPages(src, sink, pages: [0, 1]))),
                  _Op(
                      icon: Icons.delete_outline,
                      title: 'Delete page 0',
                      loading: loading,
                      onRun: () => _opToFile('Delete', 'deleted.pdf',
                          (sink) => _pdf.deletePages(src, sink, pages: [0]))),
                  _Op(
                      icon: Icons.swap_vert,
                      title: 'Reverse page order',
                      loading: loading,
                      onRun: () => runFlow('Reverse', () async {
                            final doc = await _pdf.open(src);
                            final n = doc.pageCount;
                            await doc.dispose();
                            final sink = DemoSink();
                            await _pdf.reorderPages(src, sink,
                                order: List.generate(n, (i) => n - 1 - i));
                            await saveResult(sink.takeBytes(), 'reversed.pdf');
                          })),
                  _Op(
                      icon: Icons.move_down,
                      title: 'Move page 0 → last',
                      loading: loading,
                      onRun: () => runFlow('Move', () async {
                            final doc = await _pdf.open(src);
                            final n = doc.pageCount;
                            await doc.dispose();
                            final sink = DemoSink();
                            await _pdf.movePage(src, sink, from: 0, to: n - 1);
                            await saveResult(sink.takeBytes(), 'moved.pdf');
                          })),
                  _Section('Rotation'),
                  _Op(
                      icon: Icons.rotate_right,
                      title: 'Rotate all 90°',
                      loading: loading,
                      onRun: () => _opToFile(
                          'RotateAll',
                          'rotated_all.pdf',
                          (sink) =>
                              _pdf.rotateAllPages(src, sink, degrees: 90))),
                  _Op(
                      icon: Icons.rotate_left,
                      title: 'Rotate page 0 → 180°',
                      loading: loading,
                      onRun: () => _opToFile(
                          'RotatePage',
                          'rotated_p0.pdf',
                          (sink) =>
                              _pdf.rotatePages(src, sink, pages: {0: 180}))),
                  _Section('Compression'),
                  _Op(
                      icon: Icons.compress,
                      title: 'Compress (quality 75)',
                      loading: loading,
                      onRun: () => runFlow('Compress', () async {
                            final sink = DemoSink();
                            await _pdf.compress(src, sink, imageQuality: 75);
                            final r = sink.takeBytes();
                            final pct = ((1 - r.length / bytes!.length) * 100)
                                .toStringAsFixed(1);
                            setState(() => status =
                                '${fmtSize(bytes!.length)} → ${fmtSize(r.length)} ($pct%)');
                          })),
                  _Section('Watermark'),
                  _Op(
                      icon: Icons.water_drop,
                      title: 'Center "DRAFT"',
                      loading: loading,
                      onRun: () => _opToFile(
                          'Watermark',
                          'watermarked.pdf',
                          (sink) => _pdf.watermark(src, sink,
                              text: 'DRAFT',
                              style: const PdfWatermarkStyle(opacity: 0.3)))),
                  _Op(
                      icon: Icons.grid_4x4,
                      title: 'Tiled 3×4 "COPY"',
                      loading: loading,
                      onRun: () => _opToFile(
                          'Tiled',
                          'tiled.pdf',
                          (sink) => _pdf.watermark(src, sink,
                              text: 'COPY',
                              style: const PdfWatermarkStyle(
                                  opacity: 0.15, fontSize: 24, rotation: 30),
                              position: const PdfWatermarkPosition.tiled(
                                  columns: 3, rows: 4)))),
                  _Op(
                      icon: Icons.arrow_outward,
                      title: 'Corner top-right "SAMPLE"',
                      loading: loading,
                      onRun: () => _opToFile(
                          'Corner',
                          'corner.pdf',
                          (sink) => _pdf.watermark(src, sink,
                              text: 'SAMPLE',
                              style: const PdfWatermarkStyle(
                                  opacity: 0.4, fontSize: 18, rotation: 0),
                              position: const PdfWatermarkPosition.corner(
                                  PdfCorner.topRight)))),
                  _Op(
                      icon: Icons.layers,
                      title: 'Background layer',
                      loading: loading,
                      onRun: () => _opToFile(
                          'Background',
                          'bg_watermark.pdf',
                          (sink) => _pdf.watermark(src, sink,
                              text: 'BACKGROUND',
                              style: const PdfWatermarkStyle(opacity: 0.2),
                              layer: PdfWatermarkLayer.background))),
                  _Section('Security'),
                  _Op(
                      icon: Icons.lock,
                      title: 'Encrypt (pw: secret)',
                      loading: loading,
                      onRun: () => _opToFile(
                          'Encrypt',
                          'encrypted.pdf',
                          (sink) => _pdf.encrypt(src, sink,
                              encryption: const PdfEncryptionConfig(
                                  ownerPassword: 'secret')))),
                  _Op(
                      icon: Icons.lock_open,
                      title: 'Decrypt (pw: secret)',
                      loading: loading,
                      onRun: () => _opToFile(
                          'Decrypt',
                          'decrypted.pdf',
                          (sink) =>
                              _pdf.decrypt(src, sink, password: 'secret'))),
                  _Section('Forms & Annotations'),
                  _Op(
                      icon: Icons.layers_clear,
                      title: 'Flatten forms',
                      loading: loading,
                      onRun: () => _opToFile('Flatten', 'flattened.pdf',
                          (sink) => _pdf.flattenForms(src, sink))),
                  _Op(
                      icon: Icons.rule,
                      title: 'Apply redactions',
                      loading: loading,
                      onRun: () => _opToFile('Redact', 'redacted.pdf',
                          (sink) => _pdf.applyRedactions(src, sink))),
                  _Section('Content'),
                  _Op(
                      icon: Icons.attach_file,
                      title: 'Embed text file',
                      loading: loading,
                      onRun: () => _opToFile(
                          'Embed',
                          'embedded.pdf',
                          (sink) => _pdf.embedFile(src, sink,
                              name: 'readme.txt',
                              fileData: DemoSource(Uint8List.fromList(
                                  'Hello from pdf_manipulator!'.codeUnits))))),
                  _Op(
                      icon: Icons.format_paint,
                      title: 'Erase region on page 0',
                      loading: loading,
                      onRun: () => _opToFile(
                          'Erase',
                          'erased.pdf',
                          (sink) => _pdf.eraseRegions(src, sink,
                                  page: 0,
                                  regions: [
                                    const PdfRect(
                                        x: 50, y: 700, width: 200, height: 30)
                                  ]))),
                  _Section('Stamps'),
                  _Op(
                      icon: Icons.approval,
                      title: 'Approved stamp on page 0',
                      loading: loading,
                      onRun: () => _opToFile(
                          'Stamp',
                          'stamped.pdf',
                          (sink) => _pdf.addStamp(src, sink,
                              page: 0,
                              type: PdfStampType.approved,
                              rect: const PdfRect(
                                  x: 50, y: 50, width: 200, height: 60)))),
                  _Op(
                      icon: Icons.add_photo_alternate,
                      title: 'Image stamp (pick image)',
                      loading: loading,
                      onRun: () => runFlow('ImageStamp', () async {
                            final imgs = await pickImageBytes();
                            if (imgs == null || imgs.isEmpty) return;
                            final sink = DemoSink();
                            await _pdf.addImageStamp(src, sink,
                                page: 0,
                                imageData: DemoSource(imgs.first.bytes),
                                rect: const PdfRect(
                                    x: 100, y: 100, width: 150, height: 150));
                            await saveResult(
                                sink.takeBytes(), 'image_stamped.pdf');
                          })),
                  _Section('Compliance'),
                  _Op(
                      icon: Icons.verified,
                      title: 'Convert to PDF/A',
                      loading: loading,
                      onRun: () => _opToFile('PDF/A', 'pdfa.pdf',
                          (sink) => _pdf.convertToPdfA(src, sink))),
                  _Section('Images → PDF'),
                  _Op(
                      icon: Icons.photo_library,
                      title: 'Images to PDF (pick images)',
                      loading: loading,
                      onRun: () => runFlow('ImgToPdf', () async {
                            final imgs = await pickImageBytes();
                            if (imgs == null || imgs.isEmpty) return;
                            final sink = DemoSink();
                            await _pdf.imagesToPdf(
                                imgs
                                    .map((i) =>
                                        DemoSource(i.bytes) as DataSource)
                                    .toList(),
                                sink);
                            await saveResult(
                                sink.takeBytes(), 'from_images.pdf');
                          })),
                  const SizedBox(height: 40),
                ],
              ),
      ),
    ]);
  }
}

// ─── Tab 4: PdfStandalone — source in, sink out, no handle ────────

class _StandaloneTab extends StatefulWidget {
  const _StandaloneTab();
  @override
  State<_StandaloneTab> createState() => _StandaloneTabState();
}

class _StandaloneTabState extends State<_StandaloneTab>
    with _OpsRunner, _PdfPicker, AutomaticKeepAliveClientMixin {
  final _pdf = Pdf();

  @override
  bool get wantKeepAlive => true;

  @override
  void dispose() {
    unawaited(_pdf.dispose());
    super.dispose();
  }

  void _pickFile() => unawaited(
      pick((p) => setState(() => status = 'Loaded (${fmtSize(p.length)})')));

  Future<void> _opToFile(String label, String filename,
      PdfTask<void> Function(DemoSink sink) start) async {
    final sink = DemoSink();
    if (await runTask(label, () => start(sink))) {
      await saveResult(sink.takeBytes(), filename);
    }
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Column(children: [
      statusBar(),
      Expanded(
        child: bytes == null
            ? _EmptyState(
                icon: Icons.output,
                text: 'Source in → sink out, no handle',
                onPick: _pickFile)
            : ListView(
                padding:
                    const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                children: [
                  fileCard(context, _pickFile),
                  _Section('Sign'),
                  _Op(
                      icon: Icons.verified_user_outlined,
                      title: 'Sign PDF (PEM cert)',
                      loading: loading,
                      onRun: () => _opToFile(
                          'Sign',
                          'signed.pdf',
                          (sink) => _pdf.sign(src, sink,
                              credentials: const PdfSigningCredentials.pem(
                                  testCertPem, testKeyPem)))),
                  _Section('Convert'),
                  _Op(
                      icon: Icons.description,
                      title: 'PDF → DOCX',
                      loading: loading,
                      onRun: () => _opToFile(
                          'DOCX',
                          'converted.docx',
                          (sink) => _pdf.convertTo(src, sink,
                              format: PdfDocumentFormat.docx))),
                  _Op(
                      icon: Icons.slideshow,
                      title: 'PDF → PPTX',
                      loading: loading,
                      onRun: () => _opToFile(
                          'PPTX',
                          'converted.pptx',
                          (sink) => _pdf.convertTo(src, sink,
                              format: PdfDocumentFormat.pptx))),
                  _Op(
                      icon: Icons.table_chart,
                      title: 'PDF → XLSX',
                      loading: loading,
                      onRun: () => _opToFile(
                          'XLSX',
                          'converted.xlsx',
                          (sink) => _pdf.convertTo(src, sink,
                              format: PdfDocumentFormat.xlsx))),
                  _Op(
                      icon: Icons.picture_as_pdf,
                      title: 'DOCX → PDF (round-trip)',
                      subtitle: 'Converts to DOCX first, then back to PDF',
                      loading: loading,
                      onRun: () => runFlow('DOCX→PDF', () async {
                            final docxSink = DemoSink();
                            await _pdf.convertTo(src, docxSink,
                                format: PdfDocumentFormat.docx);
                            final pdfSink = DemoSink();
                            await _pdf.convertToPdf(
                                DemoSource(docxSink.takeBytes()), pdfSink,
                                format: PdfDocumentFormat.docx);
                            await saveResult(
                                pdfSink.takeBytes(), 'roundtrip.pdf');
                          })),
                  _Section('Extract Pages'),
                  _Op(
                      icon: Icons.file_copy_outlined,
                      title: 'Extract pages 0–1',
                      loading: loading,
                      onRun: () => _opToFile(
                          'Extract',
                          'extracted_standalone.pdf',
                          (sink) =>
                              _pdf.extractPages(src, sink, pages: [0, 1]))),
                  const SizedBox(height: 40),
                ],
              ),
      ),
    ]);
  }
}

// ─── Tab 5: PdfEditor — parse once, mutate N times, save once ─────

class _EditorTab extends StatefulWidget {
  const _EditorTab();
  @override
  State<_EditorTab> createState() => _EditorTabState();
}

class _EditorTabState extends State<_EditorTab>
    with _OpsRunner, _PdfPicker, AutomaticKeepAliveClientMixin {
  final _pdf = Pdf();
  bool _emojiFontLoaded = false;

  @override
  bool get wantKeepAlive => true;

  @override
  void dispose() {
    unawaited(_pdf.dispose());
    super.dispose();
  }

  void _pickFile() => unawaited(
      pick((p) => setState(() => status = 'Loaded (${fmtSize(p.length)})')));

  /// Registers the bundled Noto Emoji face once — the instance keeps it,
  /// so repeat taps must not stack duplicate registrations.
  Future<void> _registerEmojiFont() async {
    if (_emojiFontLoaded) return;
    final font = await loadEmojiFont();
    await _pdf.registerFallbackFont(
        PdfFallbackFontKind.emoji, font.buffer.asUint8List());
    _emojiFontLoaded = true;
  }

  /// Opens an editor, runs [work], saves the produced bytes.
  Future<void> _edit(
      String label, Future<Uint8List> Function(PdfEditor e) work) async {
    await runFlow(label, () async {
      final editor = await _pdf.edit(src);
      try {
        final result = await work(editor);
        await saveResult(
            result, '${label.toLowerCase().replaceAll(' ', '_')}.pdf');
      } finally {
        await editor.dispose();
      }
    });
  }

  Future<Uint8List> _saveEditor(PdfEditor e,
      [PdfSaveOptions options = const PdfSaveOptions.fullRewrite()]) async {
    final sink = DemoSink();
    await e.save(sink, options: options);
    return sink.takeBytes();
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Column(children: [
      statusBar(),
      Expanded(
        child: bytes == null
            ? _EmptyState(
                icon: Icons.edit_document,
                text: 'Parse once, mutate N times, save once',
                onPick: _pickFile)
            : ListView(
                padding:
                    const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                children: [
                  fileCard(context, _pickFile),
                  _Section('Metadata'),
                  _Op(
                      icon: Icons.title,
                      title: 'Set title + author + subject + keywords',
                      loading: loading,
                      onRun: () => _edit('Metadata', (e) async {
                            await e.setTitle('Example');
                            await e.setAuthor('pdf_manipulator');
                            await e.setSubject('Demo');
                            await e.setKeywords('test, dart, pdf');
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.cleaning_services,
                      title: 'Scrub metadata',
                      loading: loading,
                      onRun: () => _edit('Scrub', (e) async {
                            await e.scrubMetadata();
                            return _saveEditor(e);
                          })),
                  _Section('Page Operations'),
                  _Op(
                      icon: Icons.rotate_right,
                      title: 'Rotate page 0 → 90°',
                      loading: loading,
                      onRun: () => _edit('Rotate', (e) async {
                            await e.rotatePage(0, degrees: 90);
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.rotate_right,
                      title: 'Rotate all → 90°',
                      loading: loading,
                      onRun: () => _edit('RotateAll', (e) async {
                            await e.rotateAllPages(degrees: 90);
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.delete_outline,
                      title: 'Delete last page',
                      loading: loading,
                      onRun: () => _edit('Delete', (e) async {
                            final n = await e.pageCount;
                            if (n > 1) await e.deletePage(n - 1);
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.move_down,
                      title: 'Move page 0 → 1',
                      loading: loading,
                      onRun: () => _edit('Move', (e) async {
                            if (await e.pageCount >= 2) {
                              await e.movePage(from: 0, to: 1);
                            }
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.filter_list,
                      title: 'Select pages 0,1 only',
                      loading: loading,
                      onRun: () => _edit('Select', (e) async {
                            await e.selectPages([0, 1]);
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.straighten,
                      title: 'Get page 0 media box',
                      loading: loading,
                      onRun: () => _edit('MediaBox', (e) async {
                            final r = await e.getPageMediaBox(0);
                            setState(() => status =
                                'Page 0: ${r.width.toStringAsFixed(0)}×'
                                    '${r.height.toStringAsFixed(0)} at '
                                    '(${r.x.toStringAsFixed(0)},${r.y.toStringAsFixed(0)})');
                            return _saveEditor(e);
                          })),
                  _Section('Merge'),
                  _Op(
                      icon: Icons.merge,
                      title: 'Merge with self',
                      loading: loading,
                      onRun: () => _edit('Merge', (e) async {
                            await e.mergeFrom(DemoSource(bytes!));
                            return _saveEditor(e);
                          })),
                  _Section('Optimization'),
                  _Op(
                      icon: Icons.compress,
                      title: 'Optimize images (q60)',
                      loading: loading,
                      onRun: () => _edit('OptimizeImg', (e) async {
                            final n = await e.optimizeImages(quality: 60);
                            setState(() => status = 'Optimized $n images');
                            return _saveEditor(
                                e,
                                const PdfSaveOptions.fullRewrite(
                                    compress: true, garbageCollect: true));
                          })),
                  _Op(
                      icon: Icons.font_download_off,
                      title: 'Unembed standard fonts',
                      loading: loading,
                      onRun: () => _edit('UnembedFonts', (e) async {
                            final n = await e.unembedStandardFonts();
                            setState(() => status = 'Unembedded $n fonts');
                            return _saveEditor(e);
                          })),
                  _Section('Watermark & Stamps'),
                  _Op(
                      icon: Icons.water_drop,
                      title: 'Watermark all pages "SAMPLE"',
                      loading: loading,
                      onRun: () => _edit('Watermark', (e) async {
                            final n = await e.pageCount;
                            for (var i = 0; i < n; i++) {
                              await e.addWatermark(i, 'SAMPLE',
                                  style:
                                      const PdfWatermarkStyle(opacity: 0.25));
                            }
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.approval,
                      title: 'Add stamp on page 0',
                      loading: loading,
                      onRun: () => _edit('Stamp', (e) async {
                            await e.addStamp(0,
                                type: PdfStampType.draft,
                                rect: const PdfRect(
                                    x: 50, y: 50, width: 200, height: 60));
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.add_photo_alternate,
                      title: 'Add image stamp (pick)',
                      loading: loading,
                      onRun: () => _edit('ImgStamp', (e) async {
                            final imgs = await pickImageBytes();
                            if (imgs == null || imgs.isEmpty) {
                              return _saveEditor(e);
                            }
                            await e.addImageStamp(
                                0, DemoSource(imgs.first.bytes),
                                rect: const PdfRect(
                                    x: 100, y: 100, width: 150, height: 150));
                            return _saveEditor(e);
                          })),
                  _Section('Content'),
                  _Op(
                      icon: Icons.attach_file,
                      title: 'Embed file',
                      loading: loading,
                      onRun: () => _edit('Embed', (e) async {
                            await e.embedFile(
                                'note.txt',
                                DemoSource(
                                    Uint8List.fromList('Hello!'.codeUnits)));
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.format_paint,
                      title: 'Erase region on page 0',
                      loading: loading,
                      onRun: () => _edit('Erase', (e) async {
                            await e.eraseRegions(0, [
                              const PdfRect(
                                  x: 50, y: 700, width: 200, height: 30)
                            ]);
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.crop,
                      title: 'Crop margins',
                      loading: loading,
                      onRun: () => _edit('Crop', (e) async {
                            await e.cropMargins(
                                left: 20, right: 20, top: 20, bottom: 20);
                            return _saveEditor(e);
                          })),
                  _Section('Forms'),
                  _Op(
                      icon: Icons.layers_clear,
                      title: 'Flatten forms',
                      loading: loading,
                      onRun: () => _edit('Flatten', (e) async {
                            await e.flattenForms();
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.layers_clear_outlined,
                      title: 'Flatten all annotations',
                      loading: loading,
                      onRun: () => _edit('FlattenAnnot', (e) async {
                            await e.flattenAllAnnotations();
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.edit_note,
                      title: 'Set form field value',
                      subtitle: 'fieldName="name", value="John"',
                      loading: loading,
                      onRun: () => _edit('SetField', (e) async {
                            await e.setFormFieldValue('name', 'John');
                            return _saveEditor(e);
                          })),
                  _Op(
                      icon: Icons.emoji_emotions_outlined,
                      title: 'Fill with emoji + flatten',
                      subtitle: 'registerFallbackFont bakes what the '
                          'field font cannot draw',
                      loading: loading,
                      onRun: () => _edit('EmojiFill', (e) async {
                            await _registerEmojiFont();
                            await e.setFormFieldValue(
                                'name', 'Loved it \u{1F600}\u{1F389}');
                            await e.flattenForms();
                            return _saveEditor(e);
                          })),
                  _Section('Redaction'),
                  _Op(
                      icon: Icons.remove_red_eye_outlined,
                      title: 'Add redaction + count + apply',
                      loading: loading,
                      onRun: () => _edit('Redact', (e) async {
                            await e.addRedaction(
                                0,
                                const PdfRect(
                                    x: 50, y: 700, width: 200, height: 30));
                            final n = await e.redactionCount(0);
                            setState(() => status = 'Redactions on page 0: $n');
                            await e.applyRedactions();
                            return _saveEditor(e);
                          })),
                  _Section('Compliance'),
                  _Op(
                      icon: Icons.verified,
                      title: 'Convert to PDF/A',
                      loading: loading,
                      onRun: () => _edit('PDF-A', (e) async {
                            await e.convertToPdfA();
                            return _saveEditor(e);
                          })),
                  _Section('Security'),
                  _Op(
                      icon: Icons.lock,
                      title: 'Save encrypted (pw: test)',
                      loading: loading,
                      onRun: () => _edit('Encrypt', (e) async {
                            return _saveEditor(
                                e,
                                const PdfSaveOptions.fullRewrite(
                                    encryption: PdfEncryption.config(
                                        ownerPassword: 'test')));
                          })),
                  _Op(
                      icon: Icons.lock_open,
                      title: 'Save with encryption removed',
                      loading: loading,
                      onRun: () => _edit('RemoveEncrypt', (e) async {
                            return _saveEditor(
                                e,
                                const PdfSaveOptions.fullRewrite(
                                    encryption: PdfEncryption.remove()));
                          })),
                  _Op(
                      icon: Icons.save_alt,
                      title: 'Incremental save',
                      loading: loading,
                      onRun: () => _edit('Incremental', (e) async {
                            return _saveEditor(
                                e, const PdfSaveOptions.incremental());
                          })),
                  _Section('Chained'),
                  _Op(
                      icon: Icons.auto_fix_high,
                      title: 'Rotate + watermark + compress + encrypt',
                      subtitle: 'Multiple mutations in one parse-save cycle',
                      loading: loading,
                      onRun: () => _edit('Chain', (e) async {
                            await e.rotateAllPages(degrees: 90);
                            final n = await e.pageCount;
                            for (var i = 0; i < n; i++) {
                              await e.addWatermark(i, 'PROCESSED',
                                  style: const PdfWatermarkStyle(
                                      opacity: 0.15, fontSize: 40));
                            }
                            await e.optimizeImages(quality: 70);
                            await e.setTitle('Processed');
                            return _saveEditor(
                                e,
                                const PdfSaveOptions.fullRewrite(
                                    compress: true,
                                    garbageCollect: true,
                                    encryption: PdfEncryption.config(
                                        ownerPassword: 'chain')));
                          })),
                  const SizedBox(height: 40),
                ],
              ),
      ),
    ]);
  }
}

// ─── Tab 6: PdfBuilder — create from scratch ──────────────────────

class _BuilderTab extends StatefulWidget {
  const _BuilderTab();
  @override
  State<_BuilderTab> createState() => _BuilderTabState();
}

class _BuilderTabState extends State<_BuilderTab>
    with _OpsRunner, AutomaticKeepAliveClientMixin {
  final _pdf = Pdf();

  @override
  bool get wantKeepAlive => true;

  @override
  void dispose() {
    unawaited(_pdf.dispose());
    super.dispose();
  }

  /// Builds with a fresh PdfBuilder, saves the result, always
  /// disposes the builder — even when the body throws.
  Future<void> _build(String label, String filename,
      Future<void> Function(PdfBuilder b) body) async {
    await runFlow(label, () async {
      final b = await _pdf.build();
      try {
        await body(b);
        final sink = DemoSink();
        await b.save(sink);
        await saveResult(sink.takeBytes(), filename);
      } finally {
        await b.dispose();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Column(children: [
      statusBar(),
      Expanded(
        child: ListView(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          children: [
            Card(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Row(children: [
                  Icon(Icons.note_add,
                      color: Theme.of(context).colorScheme.primary, size: 28),
                  const SizedBox(width: 12),
                  const Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text('Create PDFs from scratch',
                              style: TextStyle(
                                  fontWeight: FontWeight.w600, fontSize: 15)),
                          Text('Text, images, forms, links, watermarks',
                              style: TextStyle(fontSize: 12)),
                        ]),
                  ),
                ]),
              ),
            ),
            const SizedBox(height: 8),
            _Section('Documents'),
            _Op(
                icon: Icons.text_fields,
                title: 'Text document (A4, 2 pages)',
                subtitle: 'Headings, paragraphs, columns, horizontal rule',
                loading: loading,
                onRun: () => _build('Text', 'built_text.pdf', (b) async {
                      await b.setTitle('Example');
                      await b.setAuthor('pdf_manipulator');
                      await b.setSubject('Demo');
                      await b.setKeywords('test, dart');

                      final p1 = await b.addA4Page();
                      await p1.heading(1, 'Hello from PdfBuilder');
                      await p1.space(10);
                      await p1.paragraph(
                          'This PDF was created from Dart code. No source PDF needed.');
                      await p1.space(20);
                      await p1.heading(2, 'Features');
                      await p1.paragraph(
                          '• Text, headings, paragraphs\n• Images, watermarks\n'
                          '• Form fields, links, footnotes\n• Multi-column layout');
                      await p1.space(20);
                      await p1.horizontalRule();
                      await p1.space(10);
                      await p1.text('Page 1 of 2');
                      await p1.done();

                      final p2 = await b.addA4Page();
                      await p2.heading(1, 'Page Two');
                      await p2
                          .paragraph('Multi-page documents work seamlessly.');
                      await p2.space(20);
                      await p2.heading(3, 'Columns');
                      await p2.columns(
                          2,
                          20,
                          'This text flows across two columns. The PdfBuilder '
                          'API supports multi-column layout.');
                      await p2.done();
                    })),
            _Op(
                icon: Icons.crop_16_9,
                title: 'Letter page',
                subtitle: 'US Letter (612×792 pt)',
                loading: loading,
                onRun: () => _build('Letter', 'letter.pdf', (b) async {
                      final p = await b.addLetterPage();
                      await p.heading(1, 'US Letter Page');
                      await p.paragraph('612 × 792 points.');
                      await p.done();
                    })),
            _Op(
                icon: Icons.aspect_ratio,
                title: 'Custom size page',
                subtitle: '400×300 pt landscape',
                loading: loading,
                onRun: () => _build('Custom', 'custom_size.pdf', (b) async {
                      final p = await b.addPage(width: 400, height: 300);
                      await p.heading(1, 'Custom Size');
                      await p.text('400 × 300 points');
                      await p.done();
                    })),
            _Section('Form Fields'),
            _Op(
                icon: Icons.edit_note,
                title: 'Full form (all field types)',
                subtitle:
                    'TextField, checkbox, combo, radio, button, signature',
                loading: loading,
                onRun: () => _build('Form', 'built_form.pdf', (b) async {
                      await b.setTitle('Registration Form');

                      final p = await b.addA4Page();
                      await p.heading(1, 'Registration Form');
                      await p.space(20);
                      await p.text('Name:');
                      await p.textField('name',
                          const PdfRect(x: 72, y: 680, width: 250, height: 20),
                          defaultValue: 'John Doe');
                      await p.space(40);
                      await p.text('Email:');
                      await p.textField('email',
                          const PdfRect(x: 72, y: 630, width: 250, height: 20));
                      await p.space(40);
                      await p.text('Country:');
                      await p.comboBox(
                          'country',
                          const PdfRect(x: 72, y: 580, width: 150, height: 20),
                          ['USA', 'UK', 'India', 'Germany', 'Japan'],
                          selected: 'India');
                      await p.space(40);
                      await p.text('Terms:');
                      await p.checkbox('agree',
                          const PdfRect(x: 72, y: 530, width: 14, height: 14));
                      await p.space(40);
                      await p.text('Plan:');
                      await p.radioGroup(
                          'plan',
                          [
                            (
                              value: 'free',
                              rect: const PdfRect(
                                  x: 72, y: 480, width: 14, height: 14)
                            ),
                            (
                              value: 'pro',
                              rect: const PdfRect(
                                  x: 72, y: 460, width: 14, height: 14)
                            ),
                            (
                              value: 'enterprise',
                              rect: const PdfRect(
                                  x: 72, y: 440, width: 14, height: 14)
                            ),
                          ],
                          selected: 'pro');
                      await p.space(40);
                      await p.pushButton(
                          'submit',
                          const PdfRect(x: 72, y: 390, width: 80, height: 30),
                          'Submit');
                      await p.space(50);
                      await p.signatureField('sig',
                          const PdfRect(x: 72, y: 320, width: 200, height: 60));
                      await p.space(40);

                      // JavaScript actions on fields
                      await p.fieldKeystroke(
                          'AFNumber_Keystroke(2, 0, 0, 0, "", true)');
                      await p
                          .fieldFormat('AFNumber_Format(2, 0, 0, 0, "", true)');
                      await p.fieldValidate(
                          'AFRange_Validate(true, 0, true, 100)');
                      await p.fieldCalculate(
                          'AFSimple_Calculate("SUM", new Array("field1", "field2"))');

                      await p.done();
                    })),
            _Section('Links & References'),
            _Op(
                icon: Icons.link,
                title: 'URL link + page link + footnote',
                loading: loading,
                onRun: () => _build('Links', 'built_links.pdf', (b) async {
                      await b.setTitle('Links Demo');

                      final p1 = await b.addA4Page();
                      await p1.heading(1, 'Links & References');
                      await p1.space(10);
                      await p1.text('Visit:');
                      await p1
                          .linkUrl('https://pub.dev/packages/pdf_manipulator');
                      await p1.space(20);
                      await p1.text('Jump to page 2:');
                      await p1.linkPage(1);
                      await p1.space(20);
                      await p1.text('With a footnote.');
                      await p1.footnote('1', 'This is the footnote text.');
                      await p1.done();

                      final p2 = await b.addA4Page();
                      await p2.heading(2, 'Page 2 — Link Target');
                      await p2.paragraph('You arrived here via the page link.');
                      await p2.done();
                    })),
            _Section('Watermark & Image'),
            _Op(
                icon: Icons.water_drop,
                title: 'Page with watermark',
                loading: loading,
                onRun: () =>
                    _build('Watermark', 'built_watermark.pdf', (b) async {
                      final p = await b.addA4Page();
                      await p.heading(1, 'Confidential Report');
                      await p.paragraph('Sensitive information.');
                      await p.watermark('CONFIDENTIAL');
                      await p.done();
                    })),
            _Op(
                icon: Icons.image,
                title: 'Page with image (pick)',
                loading: loading,
                onRun: () async {
                  final imgs = await pickImageBytes();
                  if (imgs == null || imgs.isEmpty) return;
                  await _build('Image', 'built_image.pdf', (b) async {
                    final p = await b.addA4Page();
                    await p.heading(1, 'Image Demo');
                    await p.image(DemoSource(imgs.first.bytes),
                        const PdfRect(x: 72, y: 500, width: 200, height: 200));
                    await p.done();
                  });
                }),
            _Section('Multi-Page'),
            _Op(
                icon: Icons.library_books,
                title: 'newline + newPageSameSize',
                loading: loading,
                onRun: () => _build('MultiPage', 'built_multi.pdf', (b) async {
                      final p = await b.addA4Page();
                      await p.heading(1, 'Page 1');
                      await p.paragraph('Some content on page 1.');
                      await p.newline();
                      await p.text('After a newline.');
                      await p.newPageSameSize();
                      await p.heading(1, 'Page 2 (same size)');
                      await p.paragraph('Created via newPageSameSize().');
                      await p.done();
                    })),
            _Section('Font'),
            _Op(
                icon: Icons.font_download,
                title: 'Custom font + size',
                loading: loading,
                onRun: () => _build('Font', 'built_fonts.pdf', (b) async {
                      final p = await b.addA4Page();
                      await p.font('Helvetica', 24);
                      await p.text('Large Helvetica');
                      await p.font('Courier', 12);
                      await p.text('Small Courier');
                      await p.font('Times-Roman', 18);
                      await p.text('Medium Times');
                      await p.done();
                    })),
            const SizedBox(height: 40),
          ],
        ),
      ),
    ]);
  }
}

// ─── Tab 7: Merge — reorderable multi-file merge ──────────────────

class _MergeTab extends StatefulWidget {
  const _MergeTab();
  @override
  State<_MergeTab> createState() => _MergeTabState();
}

class _MergeTabState extends State<_MergeTab>
    with _OpsRunner, AutomaticKeepAliveClientMixin {
  final _pdf = Pdf();
  final _files = <({String name, Uint8List bytes, int size})>[];
  Uint8List? _merged;

  @override
  bool get wantKeepAlive => true;

  @override
  void dispose() {
    unawaited(_pdf.dispose());
    super.dispose();
  }

  Future<void> _add() async {
    final picked = await pickPdfBytes(multiple: true);
    if (picked == null) return;
    for (final bytes in picked) {
      _files.add((
        name: 'demo-${_files.length + 1}.pdf',
        bytes: bytes,
        size: bytes.length,
      ));
    }
    setState(() {
      _merged = null;
      status = '${_files.length} files';
    });
  }

  Future<void> _merge() async {
    if (_files.length < 2) return;
    final sink = DemoSink();
    final ok = await runTask(
        'Merging ${_files.length} files',
        () => _pdf.merge(
            _files.map((f) => DemoSource(f.bytes) as DataSource).toList(),
            sink));
    if (ok) {
      final r = sink.takeBytes();
      setState(() {
        _merged = r;
        status = 'Merged: ${fmtSize(r.length)}';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Column(children: [
      statusBar(),
      Expanded(
        child: _files.isEmpty
            ? _EmptyState(
                icon: Icons.merge,
                text: 'Pick 2+ PDFs to merge — drag to reorder',
                onPick: _add,
                pickLabel: 'Pick PDFs')
            : ReorderableListView.builder(
                padding: const EdgeInsets.all(16),
                itemCount: _files.length,
                // onReorderItem pre-adjusts newIndex for the removed
                // slot — no manual decrement needed.
                onReorderItem: (oldIndex, newIndex) => setState(() {
                  final item = _files.removeAt(oldIndex);
                  _files.insert(newIndex, item);
                  _merged = null;
                }),
                itemBuilder: (_, i) => Card(
                  key: ValueKey('${_files[i].name}_$i'),
                  margin: const EdgeInsets.symmetric(vertical: 3),
                  child: ListTile(
                    leading: CircleAvatar(
                        radius: 16,
                        child: Text('${i + 1}',
                            style: const TextStyle(fontSize: 13))),
                    title: Text(_files[i].name,
                        style: const TextStyle(fontSize: 14)),
                    subtitle: Text(fmtSize(_files[i].size)),
                    trailing: IconButton(
                        icon: const Icon(Icons.close, size: 18),
                        onPressed: () => setState(() {
                              _files.removeAt(i);
                              _merged = null;
                            })),
                    dense: true,
                  ),
                ),
              ),
      ),
      if (_files.isNotEmpty)
        Padding(
          padding: const EdgeInsets.all(16),
          child: Row(children: [
            OutlinedButton.icon(
                onPressed: loading ? null : _add,
                icon: const Icon(Icons.add, size: 18),
                label: const Text('Add')),
            const SizedBox(width: 8),
            Expanded(
              child: _merged != null
                  ? FilledButton.icon(
                      onPressed: () async {
                        final p = await saveBytes(_merged!, 'merged.pdf');
                        if (p != null) setState(() => status = 'Saved');
                      },
                      icon: const Icon(Icons.save, size: 18),
                      label: const Text('Save'))
                  : FilledButton.icon(
                      onPressed: _files.length >= 2 && !loading ? _merge : null,
                      icon: loading
                          ? const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(strokeWidth: 2))
                          : const Icon(Icons.merge, size: 18),
                      label: Text('Merge ${_files.length}')),
            ),
          ]),
        ),
    ]);
  }
}
