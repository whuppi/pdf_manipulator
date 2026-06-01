import 'dart:developer';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import 'package:file_picker/file_picker.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

// ─── In-memory DataSource / DataSink ──────────────────────────────

class MemorySource implements DataSource {
  MemorySource(this._data);
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

class MemorySink implements DataSink {
  final _builder = BytesBuilder(copy: false);
  @override
  void write(Uint8List chunk) => _builder.add(chunk);
  Uint8List takeBytes() => _builder.takeBytes();
  int get length => _builder.length;
}

// ─── Minimal test PDF (1 blank A4 page) ───────────────────────────

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

// ─── Test certificate for signing demos ───────────────────────────

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

Future<List<Uint8List>?> pickPdfBytes({bool multiple = false}) async {
  final result = await FilePicker.platform.pickFiles(
    allowMultiple: multiple, type: FileType.custom,
    allowedExtensions: ['pdf'], withData: true,
  );
  if (result == null) return null;
  return result.files.where((f) => f.bytes != null).map((f) => f.bytes!).toList();
}

Future<List<({String name, Uint8List bytes, int size})>?> pickImageBytes() async {
  final result = await FilePicker.platform.pickFiles(
    allowMultiple: true, type: FileType.image, withData: true,
  );
  if (result == null) return null;
  return result.files
      .where((f) => f.bytes != null)
      .map((f) => (name: f.name, bytes: f.bytes!, size: f.size))
      .toList();
}

Future<String?> saveBytes(Uint8List bytes, String name) =>
    FilePicker.platform.saveFile(dialogTitle: 'Save $name', fileName: name, bytes: bytes);

// ─── App ──────────────────────────────────────────────────────────

void main() => runApp(const MyApp());

class MyApp extends StatelessWidget {
  const MyApp({super.key});
  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'PDF Manipulator',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(useMaterial3: true, colorSchemeSeed: Colors.deepPurple),
      darkTheme: ThemeData(useMaterial3: true, brightness: Brightness.dark, colorSchemeSeed: Colors.deepPurple),
      themeMode: ThemeMode.system,
      home: const HomePage(),
    );
  }
}

class HomePage extends StatefulWidget {
  const HomePage({super.key});
  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  int _tab = 0;
  final _pages = const <Widget>[
    _DocTab(),
    _SugarTab(),
    _StandaloneTab(),
    _EditorTab(),
    _BuilderTab(),
    _MergeTab(),
  ];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: _pages[_tab],
      bottomNavigationBar: NavigationBar(
        selectedIndex: _tab,
        onDestinationSelected: (i) => setState(() => _tab = i),
        destinations: const [
          NavigationDestination(icon: Icon(Icons.description), label: 'Doc'),
          NavigationDestination(icon: Icon(Icons.auto_fix_high), label: 'Sugar'),
          NavigationDestination(icon: Icon(Icons.output), label: 'Standalone'),
          NavigationDestination(icon: Icon(Icons.edit_document), label: 'Editor'),
          NavigationDestination(icon: Icon(Icons.note_add), label: 'Builder'),
          NavigationDestination(icon: Icon(Icons.merge), label: 'Merge'),
        ],
      ),
    );
  }
}

// ─── Shared widgets ───────────────────────────────────────────────

class _Status extends StatelessWidget {
  final bool loading;
  final String? message;
  final VoidCallback? onDismiss;
  const _Status({this.loading = false, this.message, this.onDismiss});

  @override
  Widget build(BuildContext context) {
    if (!loading && message == null) return const SizedBox.shrink();
    final cs = Theme.of(context).colorScheme;
    final isErr = message?.startsWith('Error') == true;
    final bg = loading ? cs.primaryContainer : (isErr ? cs.errorContainer : cs.tertiaryContainer);
    final fg = loading ? cs.onPrimaryContainer : (isErr ? cs.onErrorContainer : cs.onTertiaryContainer);
    return Container(
      width: double.infinity, padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10), color: bg,
      child: Row(children: [
        if (loading) ...[SizedBox(width: 14, height: 14, child: CircularProgressIndicator(strokeWidth: 2, color: fg)), const SizedBox(width: 12)],
        Expanded(child: Text(message ?? '', style: TextStyle(color: fg, fontSize: 13), maxLines: 4)),
        if (!loading && onDismiss != null) GestureDetector(onTap: onDismiss, child: Icon(Icons.close, size: 16, color: fg)),
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
    child: Text(text.toUpperCase(), style: TextStyle(fontSize: 11, fontWeight: FontWeight.w700, letterSpacing: 1.2, color: Theme.of(context).colorScheme.primary)),
  );
}

class _Op extends StatelessWidget {
  final IconData icon;
  final String title;
  final String? subtitle;
  final VoidCallback onRun;
  final bool loading;
  const _Op({required this.icon, required this.title, this.subtitle, required this.onRun, this.loading = false});

  @override
  Widget build(BuildContext context) => Card(
    margin: const EdgeInsets.symmetric(vertical: 2),
    child: ListTile(
      leading: Icon(icon, size: 22),
      title: Text(title, style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500)),
      subtitle: subtitle != null ? Text(subtitle!, style: const TextStyle(fontSize: 12)) : null,
      trailing: SizedBox(width: 64, height: 34, child: FilledButton.tonal(
        onPressed: loading ? null : onRun,
        style: FilledButton.styleFrom(padding: EdgeInsets.zero, textStyle: const TextStyle(fontSize: 13)),
        child: const Text('Run'),
      )),
      dense: true, visualDensity: VisualDensity.compact,
    ),
  );
}

// ─── Tab 1: PdfDoc — read-only queries ────────────────────────────

class _DocTab extends StatefulWidget {
  const _DocTab();
  @override
  State<_DocTab> createState() => _DocTabState();
}

class _DocTabState extends State<_DocTab> {
  final _pdf = Pdf();
  Uint8List? _bytes;
  PdfDoc? _doc;
  bool _loading = false;
  String? _status;

  @override
  void dispose() { _pdf.dispose(); super.dispose(); }

  Future<void> _pick() async {
    final r = await pickPdfBytes();
    if (r == null) return;
    setState(() { _loading = true; _status = 'Opening...'; });
    try {
      _doc?.dispose();
      final doc = await _pdf.open(MemorySource(r.first));
      setState(() { _bytes = r.first; _doc = doc; _status = '${doc.pageCount} pages, v${doc.version}'; });
    } catch (e) { setState(() => _status = 'Error: $e'); }
    finally { setState(() => _loading = false); }
  }

  Future<void> _run(String label, Future<void> Function(PdfDoc doc) op) async {
    if (_doc == null) return;
    setState(() { _loading = true; _status = '$label...'; });
    try { await op(_doc!); } catch (e) { setState(() => _status = 'Error: $e'); log('$label: $e'); }
    finally { if (mounted) setState(() => _loading = false); }
  }

  void _showText(String title, String text) {
    showModalBottomSheet(context: context, isScrollControlled: true, useSafeArea: true,
      builder: (ctx) => DraggableScrollableSheet(expand: false, initialChildSize: 0.7, maxChildSize: 0.95,
        builder: (ctx, sc) => Column(children: [
          Padding(padding: const EdgeInsets.fromLTRB(16, 16, 8, 8), child: Row(children: [
            Expanded(child: Text(title, style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 16))),
            IconButton(icon: const Icon(Icons.close), onPressed: () => Navigator.pop(ctx)),
          ])),
          const Divider(height: 1),
          Expanded(child: SingleChildScrollView(controller: sc, padding: const EdgeInsets.all(16),
            child: SelectableText(text.isEmpty ? '(empty)' : text, style: const TextStyle(fontSize: 13, fontFamily: 'monospace', height: 1.5)))),
        ])));
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('PdfDoc — Read-Only Queries')),
      body: Column(children: [
        _Status(loading: _loading, message: _status, onDismiss: () => setState(() => _status = null)),
        Expanded(child: _doc == null
          ? Center(child: Column(mainAxisSize: MainAxisSize.min, children: [
              Icon(Icons.description, size: 56, color: Theme.of(context).colorScheme.primary.withValues(alpha: 0.5)),
              const SizedBox(height: 16), const Text('Open a PDF to query it'),
              const SizedBox(height: 16), FilledButton.icon(onPressed: _loading ? null : _pick, icon: const Icon(Icons.file_open), label: const Text('Open PDF')),
            ]))
          : ListView(padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8), children: [
              // Info card
              Card(child: Padding(padding: const EdgeInsets.all(16), child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                Row(children: [
                  Icon(Icons.picture_as_pdf, color: Theme.of(context).colorScheme.primary, size: 28),
                  const SizedBox(width: 12),
                  Expanded(child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                    Text('${_doc!.pageCount} pages  •  ${fmtSize(_bytes!.length)}  •  v${_doc!.version}', style: const TextStyle(fontSize: 13)),
                    if (_doc!.title != null) Text('Title: ${_doc!.title}', style: const TextStyle(fontSize: 12)),
                    if (_doc!.author != null) Text('Author: ${_doc!.author}', style: const TextStyle(fontSize: 12)),
                  ])),
                  IconButton.filledTonal(onPressed: _pick, icon: const Icon(Icons.swap_horiz, size: 20)),
                ]),
                const SizedBox(height: 8),
                Wrap(spacing: 6, runSpacing: 4, children: [
                  if (_doc!.isEncrypted) _chip('Encrypted', true),
                  if (_doc!.isTagged) _chip('Tagged', false),
                  if (_doc!.subject != null) _chip(_doc!.subject!, false),
                  if (_doc!.keywords != null) _chip(_doc!.keywords!, false),
                ]),
              ]))),

              // Page dimensions
              _Section('Page Dimensions'),
              ...List.generate(_doc!.pageCount.clamp(0, 20), (i) => Padding(
                padding: const EdgeInsets.symmetric(vertical: 2, horizontal: 12),
                child: Text('Page ${i + 1}: ${_doc!.pages[i].effectiveWidth.toStringAsFixed(0)} × ${_doc!.pages[i].effectiveHeight.toStringAsFixed(0)} pt'
                    '${_doc!.pages[i].rotation != 0 ? '  (${_doc!.pages[i].rotation}°)' : ''}', style: const TextStyle(fontSize: 13)),
              )),
              if (_doc!.pageCount > 20) Text('+ ${_doc!.pageCount - 20} more', style: const TextStyle(fontSize: 12)),

              // Text extraction
              _Section('Text Extraction'),
              _Op(icon: Icons.text_snippet, title: 'Extract all (plain text)', loading: _loading,
                onRun: () => _run('Extract', (d) async {
                  final t = await d.extract(pages: const PdfPages.all());
                  _showText('Plain Text (${t.length} chars)', t);
                  setState(() => _status = '${t.length} chars');
                })),
              _Op(icon: Icons.text_snippet_outlined, title: 'Extract page 1 only', loading: _loading,
                onRun: () => _run('Extract p1', (d) async {
                  final t = await d.extract(pages: const PdfPages.single(0));
                  _showText('Page 1', t); setState(() => _status = 'Page 1: ${t.length} chars');
                })),
              _Op(icon: Icons.code, title: 'Extract as Markdown', loading: _loading,
                onRun: () => _run('Markdown', (d) async {
                  final t = await d.extract(pages: const PdfPages.all(), format: PdfExtractionFormat.markdown);
                  _showText('Markdown', t); setState(() => _status = 'MD: ${t.length} chars');
                })),
              _Op(icon: Icons.html, title: 'Extract as HTML (page 1)', loading: _loading,
                onRun: () => _run('HTML', (d) async {
                  final t = await d.extract(pages: const PdfPages.single(0), format: PdfExtractionFormat.html);
                  _showText('HTML', t); setState(() => _status = 'HTML: ${t.length} chars');
                })),

              // Search
              _Section('Search'),
              _Op(icon: Icons.search, title: 'Search "the" on page 1', loading: _loading,
                onRun: () => _run('Search p1', (d) async {
                  final r = await d.search(query: 'the', pages: const PdfPages.single(0));
                  setState(() => _status = '${r.length} hits on page 1');
                })),
              _Op(icon: Icons.manage_search, title: 'Search "the" all pages', loading: _loading,
                onRun: () => _run('Search all', (d) async {
                  final r = await d.search(query: 'the', pages: const PdfPages.all());
                  setState(() => _status = '${r.length} hits total');
                })),

              // Render
              _Section('Render'),
              _Op(icon: Icons.image, title: 'Render page 1 to image', loading: _loading,
                onRun: () => _run('Render', (d) async {
                  var count = 0;
                  await for (final page in d.render(pages: const PdfPages.single(0))) {
                    count++;
                    setState(() => _status = 'Rendered: ${page.width}×${page.height} (${fmtSize(page.data.length)})');
                  }
                  if (count == 0) setState(() => _status = 'No pages rendered');
                })),
              _Op(icon: Icons.photo_library, title: 'Extract images from page 1', loading: _loading,
                onRun: () => _run('Images', (d) async {
                  var count = 0;
                  await for (final img in d.extractImages(pages: const PdfPages.single(0))) {
                    count++; log('Image: ${img.width}×${img.height} ${img.format}');
                  }
                  setState(() => _status = '$count image(s) on page 1');
                })),

              // Signatures
              _Section('Signatures'),
              _Op(icon: Icons.draw, title: 'List signatures', loading: _loading,
                onRun: () => _run('Sigs', (d) async {
                  final sigs = await d.getSignatures();
                  setState(() => _status = '${sigs.length} signature(s)');
                })),
              _Op(icon: Icons.verified_user, title: 'Verify signatures', loading: _loading,
                onRun: () => _run('Verify', (d) async {
                  final ok = await d.verifySignatures();
                  setState(() => _status = ok ? 'Valid ✓' : 'Invalid or none ✗');
                })),

              // Validation
              _Section('Validation'),
              _Op(icon: Icons.verified, title: 'Validate PDF/A', loading: _loading,
                onRun: () => _run('PDF/A', (d) async {
                  final r = await d.validatePdfA();
                  setState(() => _status = '${r.compliant ? "Compliant ✓" : "Not compliant"} (${r.errors}e ${r.warnings}w)');
                })),
              _Op(icon: Icons.accessibility, title: 'Validate PDF/UA', loading: _loading,
                onRun: () => _run('PDF/UA', (d) async {
                  final r = await d.validatePdfUa();
                  setState(() => _status = r ? 'Accessible ✓' : 'Not accessible ✗');
                })),

              // Classification
              _Section('Classification'),
              _Op(icon: Icons.category, title: 'Classify page 1', loading: _loading,
                onRun: () => _run('Classify page', (d) async {
                  final r = await d.classifyPage(0);
                  setState(() => _status = 'Page 1: ${r.type}');
                })),
              _Op(icon: Icons.analytics, title: 'Classify document', loading: _loading,
                onRun: () => _run('Classify doc', (d) async {
                  final r = await d.classifyDocument();
                  setState(() => _status = 'Document: ${r.type}');
                })),

              // Bookmarks
              _Section('Bookmarks'),
              _Op(icon: Icons.bookmark, title: 'Plan split by bookmarks', loading: _loading,
                onRun: () => _run('Bookmarks', (d) async {
                  try {
                    final s = await d.planSplitByBookmarks();
                    setState(() => _status = '${s.length} segments: ${s.map((x) => x.title).join(', ')}');
                  } catch (_) { setState(() => _status = 'No bookmarks'); }
                })),

              const SizedBox(height: 40),
            ]),
        ),
      ]),
    );
  }

  Widget _chip(String text, bool warn) {
    final cs = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(color: warn ? cs.errorContainer : cs.surfaceContainerHighest, borderRadius: BorderRadius.circular(12)),
      child: Text(text, style: TextStyle(fontSize: 11, color: warn ? cs.onErrorContainer : cs.onSurfaceVariant)),
    );
  }
}

// ─── Tab 2: PdfSugar — one-shot convenience ops ───────────────────

class _SugarTab extends StatefulWidget {
  const _SugarTab();
  @override
  State<_SugarTab> createState() => _SugarTabState();
}

class _SugarTabState extends State<_SugarTab> {
  final _pdf = Pdf();
  Uint8List? _bytes;
  bool _loading = false;
  String? _status;

  @override
  void dispose() { _pdf.dispose(); super.dispose(); }

  Future<void> _pick() async {
    final r = await pickPdfBytes();
    if (r == null) return;
    setState(() { _bytes = r.first; _status = 'Loaded (${fmtSize(r.first.length)})'; });
  }

  DataSource get _src => MemorySource(_bytes!);

  Future<void> _run(String label, Future<void> Function() op) async {
    setState(() { _loading = true; _status = '$label...'; });
    try { await op(); } catch (e) { setState(() => _status = 'Error: $e'); }
    finally { if (mounted) setState(() => _loading = false); }
  }

  Future<void> _save(Uint8List result, String name) async {
    final p = await saveBytes(result, name);
    setState(() => _status = p != null ? 'Saved $name (${fmtSize(result.length)})' : 'Cancelled');
  }

  @override
  Widget build(BuildContext context) {
    final has = _bytes != null;
    return Scaffold(
      appBar: AppBar(title: const Text('PdfSugar — One-Shot Ops')),
      body: Column(children: [
        _Status(loading: _loading, message: _status, onDismiss: () => setState(() => _status = null)),
        Expanded(child: !has
          ? Center(child: Column(mainAxisSize: MainAxisSize.min, children: [
              Icon(Icons.auto_fix_high, size: 56, color: Theme.of(context).colorScheme.primary.withValues(alpha: 0.5)),
              const SizedBox(height: 16), const Text('Pick a PDF for sugar ops'),
              const SizedBox(height: 16), FilledButton.icon(onPressed: _pick, icon: const Icon(Icons.file_open), label: const Text('Open PDF')),
            ]))
          : ListView(padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8), children: [
              Card(child: ListTile(
                leading: Icon(Icons.picture_as_pdf, color: Theme.of(context).colorScheme.primary),
                title: Text(fmtSize(_bytes!.length)), trailing: IconButton.filledTonal(onPressed: _pick, icon: const Icon(Icons.swap_horiz, size: 20)),
              )),

              _Section('Structural'),
              _Op(icon: Icons.content_cut, title: 'Split every 2 pages', loading: _loading,
                onRun: () => _run('Split', () async {
                  final sinks = <MemorySink>[];
                  await _pdf.split(_src, (i) { final s = MemorySink(); sinks.add(s); return s; }, every: 2);
                  setState(() => _status = '${sinks.length} chunks: ${sinks.map((s) => fmtSize(s.length)).join(', ')}');
                })),
              _Op(icon: Icons.straighten, title: 'Split by size (500 KB)', loading: _loading,
                onRun: () => _run('SplitBySize', () async {
                  final sinks = <MemorySink>[];
                  await _pdf.splitBySize(_src, (i) { final s = MemorySink(); sinks.add(s); return s; }, maxBytes: 500000);
                  setState(() => _status = '${sinks.length} chunks: ${sinks.map((s) => fmtSize(s.length)).join(', ')}');
                })),
              _Op(icon: Icons.call_split, title: 'Split by bookmarks', loading: _loading,
                onRun: () => _run('SplitBookmarks', () async {
                  try {
                    final sinks = <MemorySink>[];
                    await _pdf.splitByBookmarks(_src, (i) { final s = MemorySink(); sinks.add(s); return s; });
                    setState(() => _status = '${sinks.length} parts');
                  } catch (_) { setState(() => _status = 'No bookmarks'); }
                })),
              _Op(icon: Icons.file_copy_outlined, title: 'Extract pages 0–1', loading: _loading,
                onRun: () => _run('Extract', () async {
                  final sink = MemorySink(); await _pdf.extractPages(_src, sink, pages: [0, 1]);
                  await _save(sink.takeBytes(), 'extracted.pdf');
                })),
              _Op(icon: Icons.delete_outline, title: 'Delete page 0', loading: _loading,
                onRun: () => _run('Delete', () async {
                  final sink = MemorySink(); await _pdf.deletePages(_src, sink, pages: [0]);
                  await _save(sink.takeBytes(), 'deleted.pdf');
                })),
              _Op(icon: Icons.swap_vert, title: 'Reverse page order', loading: _loading,
                onRun: () => _run('Reorder', () async {
                  final doc = await _pdf.open(_src); final n = doc.pageCount; await doc.dispose();
                  final sink = MemorySink();
                  await _pdf.reorderPages(_src, sink, order: List.generate(n, (i) => n - 1 - i));
                  await _save(sink.takeBytes(), 'reversed.pdf');
                })),
              _Op(icon: Icons.move_down, title: 'Move page 0 → last', loading: _loading,
                onRun: () => _run('Move', () async {
                  final doc = await _pdf.open(_src); final n = doc.pageCount; await doc.dispose();
                  final sink = MemorySink(); await _pdf.movePage(_src, sink, from: 0, to: n - 1);
                  await _save(sink.takeBytes(), 'moved.pdf');
                })),

              _Section('Rotation'),
              _Op(icon: Icons.rotate_right, title: 'Rotate all 90°', loading: _loading,
                onRun: () => _run('RotateAll', () async {
                  final sink = MemorySink(); await _pdf.rotateAllPages(_src, sink, degrees: 90);
                  await _save(sink.takeBytes(), 'rotated_all.pdf');
                })),
              _Op(icon: Icons.rotate_left, title: 'Rotate page 0 → 180°', loading: _loading,
                onRun: () => _run('RotatePage', () async {
                  final sink = MemorySink(); await _pdf.rotatePages(_src, sink, pages: {0: 180});
                  await _save(sink.takeBytes(), 'rotated_p0.pdf');
                })),

              _Section('Compression'),
              _Op(icon: Icons.compress, title: 'Compress (quality 75)', loading: _loading,
                onRun: () => _run('Compress', () async {
                  final sink = MemorySink(); await _pdf.compress(_src, sink, imageQuality: 75);
                  final r = sink.takeBytes();
                  final pct = ((1 - r.length / _bytes!.length) * 100).toStringAsFixed(1);
                  setState(() => _status = '${fmtSize(_bytes!.length)} → ${fmtSize(r.length)} ($pct%)');
                })),

              _Section('Watermark'),
              _Op(icon: Icons.water_drop, title: 'Center "DRAFT"', loading: _loading,
                onRun: () => _run('Watermark', () async {
                  final sink = MemorySink();
                  await _pdf.watermark(_src, sink, text: 'DRAFT', style: const PdfWatermarkStyle(opacity: 0.3));
                  await _save(sink.takeBytes(), 'watermarked.pdf');
                })),
              _Op(icon: Icons.grid_4x4, title: 'Tiled 3×4 "COPY"', loading: _loading,
                onRun: () => _run('Tiled', () async {
                  final sink = MemorySink();
                  await _pdf.watermark(_src, sink, text: 'COPY',
                    style: const PdfWatermarkStyle(opacity: 0.15, fontSize: 24, rotation: 30),
                    position: const PdfWatermarkPosition.tiled(columns: 3, rows: 4));
                  await _save(sink.takeBytes(), 'tiled.pdf');
                })),
              _Op(icon: Icons.arrow_outward, title: 'Corner top-right "SAMPLE"', loading: _loading,
                onRun: () => _run('Corner', () async {
                  final sink = MemorySink();
                  await _pdf.watermark(_src, sink, text: 'SAMPLE',
                    style: const PdfWatermarkStyle(opacity: 0.4, fontSize: 18, rotation: 0),
                    position: const PdfWatermarkPosition.corner(PdfCorner.topRight));
                  await _save(sink.takeBytes(), 'corner.pdf');
                })),
              _Op(icon: Icons.layers, title: 'Background layer', loading: _loading,
                onRun: () => _run('Background', () async {
                  final sink = MemorySink();
                  await _pdf.watermark(_src, sink, text: 'BACKGROUND',
                    style: const PdfWatermarkStyle(opacity: 0.2), layer: PdfWatermarkLayer.background);
                  await _save(sink.takeBytes(), 'bg_watermark.pdf');
                })),

              _Section('Security'),
              _Op(icon: Icons.lock, title: 'Encrypt (pw: secret)', loading: _loading,
                onRun: () => _run('Encrypt', () async {
                  final sink = MemorySink();
                  await _pdf.encrypt(_src, sink, encryption: const PdfEncryptionConfig(ownerPassword: 'secret'));
                  await _save(sink.takeBytes(), 'encrypted.pdf');
                })),
              _Op(icon: Icons.lock_open, title: 'Decrypt (pw: secret)', loading: _loading,
                onRun: () => _run('Decrypt', () async {
                  final sink = MemorySink();
                  await _pdf.decrypt(_src, sink, password: 'secret');
                  await _save(sink.takeBytes(), 'decrypted.pdf');
                })),

              _Section('Forms & Annotations'),
              _Op(icon: Icons.layers_clear, title: 'Flatten forms', loading: _loading,
                onRun: () => _run('Flatten', () async {
                  final sink = MemorySink(); await _pdf.flattenForms(_src, sink);
                  await _save(sink.takeBytes(), 'flattened.pdf');
                })),
              _Op(icon: Icons.rule, title: 'Apply redactions', loading: _loading,
                onRun: () => _run('Redact', () async {
                  final sink = MemorySink(); await _pdf.applyRedactions(_src, sink);
                  await _save(sink.takeBytes(), 'redacted.pdf');
                })),

              _Section('Content'),
              _Op(icon: Icons.attach_file, title: 'Embed text file', loading: _loading,
                onRun: () => _run('Embed', () async {
                  final sink = MemorySink();
                  await _pdf.embedFile(_src, sink, name: 'readme.txt',
                    fileData: MemorySource(Uint8List.fromList('Hello from pdf_manipulator!'.codeUnits)));
                  await _save(sink.takeBytes(), 'embedded.pdf');
                })),
              _Op(icon: Icons.format_paint, title: 'Erase region on page 0', loading: _loading,
                onRun: () => _run('Erase', () async {
                  final sink = MemorySink();
                  await _pdf.eraseRegions(_src, sink, page: 0, regions: [const PdfRect(x: 50, y: 700, width: 200, height: 30)]);
                  await _save(sink.takeBytes(), 'erased.pdf');
                })),

              _Section('Stamps'),
              _Op(icon: Icons.approval, title: 'Approved stamp on page 0', loading: _loading,
                onRun: () => _run('Stamp', () async {
                  final sink = MemorySink();
                  await _pdf.addStamp(_src, sink, page: 0, type: PdfStampType.approved,
                    rect: const PdfRect(x: 50, y: 50, width: 200, height: 60));
                  await _save(sink.takeBytes(), 'stamped.pdf');
                })),
              _Op(icon: Icons.add_photo_alternate, title: 'Image stamp (pick image)', loading: _loading,
                onRun: () => _run('ImageStamp', () async {
                  final imgs = await pickImageBytes();
                  if (imgs == null || imgs.isEmpty) return;
                  final sink = MemorySink();
                  await _pdf.addImageStamp(_src, sink, page: 0, imageData: MemorySource(imgs.first.bytes),
                    rect: const PdfRect(x: 100, y: 100, width: 150, height: 150));
                  await _save(sink.takeBytes(), 'image_stamped.pdf');
                })),

              _Section('Compliance'),
              _Op(icon: Icons.verified, title: 'Convert to PDF/A', loading: _loading,
                onRun: () => _run('PDF/A', () async {
                  final sink = MemorySink(); await _pdf.convertToPdfA(_src, sink);
                  await _save(sink.takeBytes(), 'pdfa.pdf');
                })),

              _Section('Images → PDF'),
              _Op(icon: Icons.photo_library, title: 'Images to PDF (pick images)', loading: _loading,
                onRun: () => _run('ImgToPdf', () async {
                  final imgs = await pickImageBytes();
                  if (imgs == null || imgs.isEmpty) return;
                  final sink = MemorySink();
                  await _pdf.imagesToPdf(imgs.map((i) => MemorySource(i.bytes) as DataSource).toList(), sink);
                  await _save(sink.takeBytes(), 'from_images.pdf');
                })),

              const SizedBox(height: 40),
            ]),
        ),
      ]),
    );
  }
}

// ─── Tab 3: PdfStandalone — source in, sink out, no handle ────────

class _StandaloneTab extends StatefulWidget {
  const _StandaloneTab();
  @override
  State<_StandaloneTab> createState() => _StandaloneTabState();
}

class _StandaloneTabState extends State<_StandaloneTab> {
  final _pdf = Pdf();
  Uint8List? _bytes;
  bool _loading = false;
  String? _status;

  @override
  void dispose() { _pdf.dispose(); super.dispose(); }

  Future<void> _pick() async {
    final r = await pickPdfBytes();
    if (r == null) return;
    setState(() { _bytes = r.first; _status = 'Loaded (${fmtSize(r.first.length)})'; });
  }

  DataSource get _src => MemorySource(_bytes!);

  Future<void> _run(String label, Future<void> Function() op) async {
    setState(() { _loading = true; _status = '$label...'; });
    try { await op(); } catch (e) { setState(() => _status = 'Error: $e'); }
    finally { if (mounted) setState(() => _loading = false); }
  }

  Future<void> _save(Uint8List result, String name) async {
    final p = await saveBytes(result, name);
    setState(() => _status = p != null ? 'Saved $name (${fmtSize(result.length)})' : 'Cancelled');
  }

  @override
  Widget build(BuildContext context) {
    final has = _bytes != null;
    return Scaffold(
      appBar: AppBar(title: const Text('PdfStandalone — No Handle')),
      body: Column(children: [
        _Status(loading: _loading, message: _status, onDismiss: () => setState(() => _status = null)),
        Expanded(child: !has
          ? Center(child: Column(mainAxisSize: MainAxisSize.min, children: [
              Icon(Icons.output, size: 56, color: Theme.of(context).colorScheme.primary.withValues(alpha: 0.5)),
              const SizedBox(height: 16), const Text('Source in → sink out, no handle'),
              const SizedBox(height: 16), FilledButton.icon(onPressed: _pick, icon: const Icon(Icons.file_open), label: const Text('Open PDF')),
            ]))
          : ListView(padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8), children: [
              Card(child: ListTile(
                leading: Icon(Icons.picture_as_pdf, color: Theme.of(context).colorScheme.primary),
                title: Text(fmtSize(_bytes!.length)), trailing: IconButton.filledTonal(onPressed: _pick, icon: const Icon(Icons.swap_horiz, size: 20)),
              )),

              _Section('Sign'),
              _Op(icon: Icons.verified_user_outlined, title: 'Sign PDF (PEM cert)', loading: _loading,
                onRun: () => _run('Sign', () async {
                  final sink = MemorySink();
                  await _pdf.sign(_src, sink, credentials: const PdfSigningCredentials.pem(testCertPem, testKeyPem));
                  await _save(sink.takeBytes(), 'signed.pdf');
                })),

              _Section('Convert'),
              _Op(icon: Icons.description, title: 'PDF → DOCX', loading: _loading,
                onRun: () => _run('DOCX', () async {
                  final sink = MemorySink();
                  await _pdf.convertTo(_src, sink, format: PdfDocumentFormat.docx);
                  await _save(sink.takeBytes(), 'converted.docx');
                })),
              _Op(icon: Icons.slideshow, title: 'PDF → PPTX', loading: _loading,
                onRun: () => _run('PPTX', () async {
                  final sink = MemorySink();
                  await _pdf.convertTo(_src, sink, format: PdfDocumentFormat.pptx);
                  await _save(sink.takeBytes(), 'converted.pptx');
                })),
              _Op(icon: Icons.table_chart, title: 'PDF → XLSX', loading: _loading,
                onRun: () => _run('XLSX', () async {
                  final sink = MemorySink();
                  await _pdf.convertTo(_src, sink, format: PdfDocumentFormat.xlsx);
                  await _save(sink.takeBytes(), 'converted.xlsx');
                })),
              _Op(icon: Icons.picture_as_pdf, title: 'DOCX → PDF (round-trip)', loading: _loading, subtitle: 'Converts to DOCX first, then back to PDF',
                onRun: () => _run('DOCX→PDF', () async {
                  final docxSink = MemorySink();
                  await _pdf.convertTo(_src, docxSink, format: PdfDocumentFormat.docx);
                  final pdfSink = MemorySink();
                  await _pdf.convertToPdf(MemorySource(docxSink.takeBytes()), pdfSink, format: PdfDocumentFormat.docx);
                  await _save(pdfSink.takeBytes(), 'roundtrip.pdf');
                })),

              _Section('Extract Pages'),
              _Op(icon: Icons.file_copy_outlined, title: 'Extract pages 0–1', loading: _loading,
                onRun: () => _run('Extract', () async {
                  final sink = MemorySink();
                  await _pdf.extractPages(_src, sink, pages: [0, 1]);
                  await _save(sink.takeBytes(), 'extracted_standalone.pdf');
                })),

              const SizedBox(height: 40),
            ]),
        ),
      ]),
    );
  }
}

// ─── Tab 4: PdfEditor — batch mutations ───────────────────────────

class _EditorTab extends StatefulWidget {
  const _EditorTab();
  @override
  State<_EditorTab> createState() => _EditorTabState();
}

class _EditorTabState extends State<_EditorTab> {
  final _pdf = Pdf();
  Uint8List? _bytes;
  bool _loading = false;
  String? _status;

  @override
  void dispose() { _pdf.dispose(); super.dispose(); }

  Future<void> _pick() async {
    final r = await pickPdfBytes();
    if (r == null) return;
    setState(() { _bytes = r.first; _status = 'Loaded (${fmtSize(r.first.length)})'; });
  }

  Future<void> _run(String label, Future<Uint8List> Function(PdfEditor e) work) async {
    if (_bytes == null) return;
    setState(() { _loading = true; _status = '$label...'; });
    try {
      final editor = await _pdf.edit(MemorySource(_bytes!));
      try {
        final result = await work(editor);
        final p = await saveBytes(result, '${label.toLowerCase().replaceAll(' ', '_')}.pdf');
        setState(() => _status = p != null ? 'Saved (${fmtSize(result.length)})' : 'Cancelled');
      } finally { editor.dispose(); }
    } catch (e) { setState(() => _status = 'Error: $e'); }
    finally { if (mounted) setState(() => _loading = false); }
  }

  Future<Uint8List> _saveEditor(PdfEditor e, [PdfSaveOptions options = const PdfSaveOptions.fullRewrite()]) async {
    final sink = MemorySink(); await e.save(sink, options: options); return sink.takeBytes();
  }

  @override
  Widget build(BuildContext context) {
    final has = _bytes != null;
    return Scaffold(
      appBar: AppBar(title: const Text('PdfEditor — Batch Mutations')),
      body: Column(children: [
        _Status(loading: _loading, message: _status, onDismiss: () => setState(() => _status = null)),
        Expanded(child: !has
          ? Center(child: Column(mainAxisSize: MainAxisSize.min, children: [
              Icon(Icons.edit_document, size: 56, color: Theme.of(context).colorScheme.primary.withValues(alpha: 0.5)),
              const SizedBox(height: 16), const Text('Parse once, mutate N times, save once'),
              const SizedBox(height: 16), FilledButton.icon(onPressed: _pick, icon: const Icon(Icons.file_open), label: const Text('Open PDF')),
            ]))
          : ListView(padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8), children: [
              Card(child: ListTile(
                leading: Icon(Icons.picture_as_pdf, color: Theme.of(context).colorScheme.primary),
                title: Text(fmtSize(_bytes!.length)), trailing: IconButton.filledTonal(onPressed: _pick, icon: const Icon(Icons.swap_horiz, size: 20)),
              )),

              _Section('Metadata'),
              _Op(icon: Icons.title, title: 'Set title + author + subject + keywords', loading: _loading,
                onRun: () => _run('Metadata', (e) async {
                  await e.setTitle('Example'); await e.setAuthor('pdf_manipulator');
                  await e.setSubject('Demo'); await e.setKeywords('test, dart, pdf');
                  final t = await e.getTitle(); final a = await e.getAuthor();
                  final s = await e.getSubject(); final k = await e.getKeywords();
                  log('Title=$t Author=$a Subject=$s Keywords=$k');
                  log('isModified=${await e.isModified}');
                  return _saveEditor(e);
                })),
              _Op(icon: Icons.cleaning_services, title: 'Scrub metadata', loading: _loading,
                onRun: () => _run('Scrub', (e) async { await e.scrubMetadata(); return _saveEditor(e); })),

              _Section('Page Operations'),
              _Op(icon: Icons.rotate_right, title: 'Rotate page 0 → 90°', loading: _loading,
                onRun: () => _run('Rotate', (e) async { await e.rotatePage(0, degrees: 90); return _saveEditor(e); })),
              _Op(icon: Icons.rotate_right, title: 'Rotate all → 90°', loading: _loading,
                onRun: () => _run('RotateAll', (e) async { await e.rotateAllPages(degrees: 90); return _saveEditor(e); })),
              _Op(icon: Icons.delete_outline, title: 'Delete last page', loading: _loading,
                onRun: () => _run('Delete', (e) async {
                  final n = await e.pageCount; if (n > 1) await e.deletePage(n - 1); return _saveEditor(e);
                })),
              _Op(icon: Icons.move_down, title: 'Move page 0 → 1', loading: _loading,
                onRun: () => _run('Move', (e) async {
                  if (await e.pageCount >= 2) await e.movePage(from: 0, to: 1); return _saveEditor(e);
                })),
              _Op(icon: Icons.filter_list, title: 'Select pages 0,1 only', loading: _loading,
                onRun: () => _run('Select', (e) async { await e.selectPages([0, 1]); return _saveEditor(e); })),
              _Op(icon: Icons.straighten, title: 'Get page 0 media box', loading: _loading,
                onRun: () => _run('MediaBox', (e) async {
                  final r = await e.getPageMediaBox(0);
                  setState(() => _status = 'Page 0: ${r.width.toStringAsFixed(0)}×${r.height.toStringAsFixed(0)} at (${r.x.toStringAsFixed(0)},${r.y.toStringAsFixed(0)})');
                  return _saveEditor(e);
                })),

              _Section('Merge'),
              _Op(icon: Icons.merge, title: 'Merge with self', loading: _loading,
                onRun: () => _run('Merge', (e) async {
                  await e.mergeFrom(MemorySource(_bytes!)); return _saveEditor(e);
                })),

              _Section('Optimization'),
              _Op(icon: Icons.compress, title: 'Optimize images (q60)', loading: _loading,
                onRun: () => _run('OptimizeImg', (e) async {
                  final n = await e.optimizeImages(quality: 60);
                  log('Optimized $n images');
                  return _saveEditor(e, const PdfSaveOptions.fullRewrite(compress: true, garbageCollect: true));
                })),
              _Op(icon: Icons.font_download_off, title: 'Unembed standard fonts', loading: _loading,
                onRun: () => _run('UnembedFonts', (e) async {
                  final n = await e.unembedStandardFonts();
                  log('Unembedded $n fonts'); return _saveEditor(e);
                })),

              _Section('Watermark & Stamps'),
              _Op(icon: Icons.water_drop, title: 'Watermark all pages "SAMPLE"', loading: _loading,
                onRun: () => _run('Watermark', (e) async {
                  final n = await e.pageCount;
                  for (var i = 0; i < n; i++) {
                    await e.addWatermark(i, 'SAMPLE', style: const PdfWatermarkStyle(opacity: 0.25));
                  }
                  return _saveEditor(e);
                })),
              _Op(icon: Icons.approval, title: 'Add stamp on page 0', loading: _loading,
                onRun: () => _run('Stamp', (e) async {
                  await e.addStamp(0, type: PdfStampType.draft, rect: const PdfRect(x: 50, y: 50, width: 200, height: 60));
                  return _saveEditor(e);
                })),
              _Op(icon: Icons.add_photo_alternate, title: 'Add image stamp (pick)', loading: _loading,
                onRun: () => _run('ImgStamp', (e) async {
                  final imgs = await pickImageBytes();
                  if (imgs == null || imgs.isEmpty) return _saveEditor(e);
                  await e.addImageStamp(0, MemorySource(imgs.first.bytes), rect: const PdfRect(x: 100, y: 100, width: 150, height: 150));
                  return _saveEditor(e);
                })),

              _Section('Content'),
              _Op(icon: Icons.attach_file, title: 'Embed file', loading: _loading,
                onRun: () => _run('Embed', (e) async {
                  await e.embedFile('note.txt', MemorySource(Uint8List.fromList('Hello!'.codeUnits)));
                  return _saveEditor(e);
                })),
              _Op(icon: Icons.format_paint, title: 'Erase region on page 0', loading: _loading,
                onRun: () => _run('Erase', (e) async {
                  await e.eraseRegions(0, [const PdfRect(x: 50, y: 700, width: 200, height: 30)]);
                  return _saveEditor(e);
                })),
              _Op(icon: Icons.crop, title: 'Crop margins', loading: _loading,
                onRun: () => _run('Crop', (e) async {
                  await e.cropMargins(left: 20, right: 20, top: 20, bottom: 20); return _saveEditor(e);
                })),

              _Section('Forms'),
              _Op(icon: Icons.layers_clear, title: 'Flatten forms', loading: _loading,
                onRun: () => _run('Flatten', (e) async { await e.flattenForms(); return _saveEditor(e); })),
              _Op(icon: Icons.layers_clear_outlined, title: 'Flatten all annotations', loading: _loading,
                onRun: () => _run('FlattenAnnot', (e) async { await e.flattenAllAnnotations(); return _saveEditor(e); })),
              _Op(icon: Icons.edit_note, title: 'Set form field value', loading: _loading, subtitle: 'fieldName="name", value="John"',
                onRun: () => _run('SetField', (e) async {
                  await e.setFormFieldValue('name', 'John'); return _saveEditor(e);
                })),

              _Section('Redaction'),
              _Op(icon: Icons.remove_red_eye_outlined, title: 'Add redaction + count + apply', loading: _loading,
                onRun: () => _run('Redact', (e) async {
                  await e.addRedaction(0, const PdfRect(x: 50, y: 700, width: 200, height: 30));
                  final n = await e.redactionCount(0);
                  log('Redaction count on page 0: $n');
                  await e.applyRedactions();
                  return _saveEditor(e);
                })),

              _Section('Compliance'),
              _Op(icon: Icons.verified, title: 'Convert to PDF/A', loading: _loading,
                onRun: () => _run('PDF/A', (e) async { await e.convertToPdfA(); return _saveEditor(e); })),

              _Section('Security'),
              _Op(icon: Icons.lock, title: 'Save encrypted (pw: test)', loading: _loading,
                onRun: () => _run('Encrypt', (e) async {
                  return _saveEditor(e, const PdfSaveOptions.fullRewrite(encryption: PdfEncryption.config(ownerPassword: 'test')));
                })),
              _Op(icon: Icons.lock_open, title: 'Save with encryption removed', loading: _loading,
                onRun: () => _run('RemoveEncrypt', (e) async {
                  return _saveEditor(e, const PdfSaveOptions.fullRewrite(encryption: PdfEncryption.remove()));
                })),
              _Op(icon: Icons.save_alt, title: 'Incremental save', loading: _loading,
                onRun: () => _run('Incremental', (e) async { return _saveEditor(e, const PdfSaveOptions.incremental()); })),

              _Section('Chained'),
              _Op(icon: Icons.auto_fix_high, title: 'Rotate + watermark + compress + encrypt', loading: _loading, subtitle: 'Multiple mutations in one parse-save cycle',
                onRun: () => _run('Chain', (e) async {
                  await e.rotateAllPages(degrees: 90);
                  final n = await e.pageCount;
                  for (var i = 0; i < n; i++) {
                    await e.addWatermark(i, 'PROCESSED', style: const PdfWatermarkStyle(opacity: 0.15, fontSize: 40));
                  }
                  await e.optimizeImages(quality: 70);
                  await e.setTitle('Processed');
                  return _saveEditor(e, const PdfSaveOptions.fullRewrite(compress: true, garbageCollect: true,
                    encryption: PdfEncryption.config(ownerPassword: 'chain')));
                })),

              const SizedBox(height: 40),
            ]),
        ),
      ]),
    );
  }
}

// ─── Tab 5: PdfBuilder — create from scratch ──────────────────────

class _BuilderTab extends StatefulWidget {
  const _BuilderTab();
  @override
  State<_BuilderTab> createState() => _BuilderTabState();
}

class _BuilderTabState extends State<_BuilderTab> {
  final _pdf = Pdf();
  bool _loading = false;
  String? _status;

  @override
  void dispose() { _pdf.dispose(); super.dispose(); }

  Future<void> _run(String label, Future<void> Function() op) async {
    setState(() { _loading = true; _status = '$label...'; });
    try { await op(); } catch (e) { setState(() => _status = 'Error: $e'); }
    finally { if (mounted) setState(() => _loading = false); }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('PdfBuilder — Create from Scratch')),
      body: Column(children: [
        _Status(loading: _loading, message: _status, onDismiss: () => setState(() => _status = null)),
        Expanded(child: ListView(padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8), children: [
          Card(child: Padding(padding: const EdgeInsets.all(16), child: Row(children: [
            Icon(Icons.note_add, color: Theme.of(context).colorScheme.primary, size: 28),
            const SizedBox(width: 12),
            const Expanded(child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text('Create PDFs from scratch', style: TextStyle(fontWeight: FontWeight.w600, fontSize: 15)),
              Text('Text, images, forms, links, watermarks', style: TextStyle(fontSize: 12, color: Colors.grey)),
            ])),
          ]))),
          const SizedBox(height: 8),

          _Section('Documents'),
          _Op(icon: Icons.text_fields, title: 'Text document (A4, 2 pages)', loading: _loading, subtitle: 'Headings, paragraphs, columns, horizontal rule',
            onRun: () => _run('Text', () async {
              final b = await _pdf.build();
              await b.setTitle('Example'); await b.setAuthor('pdf_manipulator');
              await b.setSubject('Demo'); await b.setKeywords('test, dart');

              final p1 = await b.addA4Page();
              await p1.heading(1, 'Hello from PdfBuilder');
              await p1.space(10);
              await p1.paragraph('This PDF was created from Dart code. No source PDF needed.');
              await p1.space(20);
              await p1.heading(2, 'Features');
              await p1.paragraph('• Text, headings, paragraphs\n• Images, watermarks\n• Form fields, links, footnotes\n• Multi-column layout');
              await p1.space(20);
              await p1.horizontalRule();
              await p1.space(10);
              await p1.text('Page 1 of 2');
              await p1.done();

              final p2 = await b.addA4Page();
              await p2.heading(1, 'Page Two');
              await p2.paragraph('Multi-page documents work seamlessly.');
              await p2.space(20);
              await p2.heading(3, 'Columns');
              await p2.columns(2, 20, 'This text flows across two columns. The PdfBuilder API supports multi-column layout.');
              await p2.done();

              final sink = MemorySink(); await b.save(sink); await b.dispose();
              final r = sink.takeBytes();
              final p = await saveBytes(r, 'built_text.pdf');
              setState(() => _status = p != null ? 'Saved (${fmtSize(r.length)})' : 'Cancelled');
            })),

          _Op(icon: Icons.crop_16_9, title: 'Letter page', loading: _loading, subtitle: 'US Letter (612×792 pt)',
            onRun: () => _run('Letter', () async {
              final b = await _pdf.build();
              final p = await b.addLetterPage();
              await p.heading(1, 'US Letter Page');
              await p.paragraph('612 × 792 points.');
              await p.done();
              final sink = MemorySink(); await b.save(sink); await b.dispose();
              final r = sink.takeBytes();
              await saveBytes(r, 'letter.pdf');
              setState(() => _status = 'Saved (${fmtSize(r.length)})');
            })),

          _Op(icon: Icons.aspect_ratio, title: 'Custom size page', loading: _loading, subtitle: '400×300 pt landscape',
            onRun: () => _run('Custom', () async {
              final b = await _pdf.build();
              final p = await b.addPage(width: 400, height: 300);
              await p.heading(1, 'Custom Size');
              await p.text('400 × 300 points');
              await p.done();
              final sink = MemorySink(); await b.save(sink); await b.dispose();
              final r = sink.takeBytes();
              await saveBytes(r, 'custom_size.pdf');
              setState(() => _status = 'Saved (${fmtSize(r.length)})');
            })),

          _Section('Form Fields'),
          _Op(icon: Icons.edit_note, title: 'Full form (all field types)', loading: _loading,
            subtitle: 'TextField, checkbox, combo, radio, button, signature',
            onRun: () => _run('Form', () async {
              final b = await _pdf.build();
              await b.setTitle('Registration Form');

              final p = await b.addA4Page();
              await p.heading(1, 'Registration Form');
              await p.space(20);
              await p.text('Name:');
              await p.textField('name', const PdfRect(x: 72, y: 680, width: 250, height: 20), defaultValue: 'John Doe');
              await p.space(40);
              await p.text('Email:');
              await p.textField('email', const PdfRect(x: 72, y: 630, width: 250, height: 20));
              await p.space(40);
              await p.text('Country:');
              await p.comboBox('country', const PdfRect(x: 72, y: 580, width: 150, height: 20), ['USA', 'UK', 'India', 'Germany', 'Japan'], selected: 'India');
              await p.space(40);
              await p.text('Terms:');
              await p.checkbox('agree', const PdfRect(x: 72, y: 530, width: 14, height: 14));
              await p.space(40);
              await p.text('Plan:');
              await p.radioGroup('plan', [
                (value: 'free', rect: const PdfRect(x: 72, y: 480, width: 14, height: 14)),
                (value: 'pro', rect: const PdfRect(x: 72, y: 460, width: 14, height: 14)),
                (value: 'enterprise', rect: const PdfRect(x: 72, y: 440, width: 14, height: 14)),
              ], selected: 'pro');
              await p.space(40);
              await p.pushButton('submit', const PdfRect(x: 72, y: 390, width: 80, height: 30), 'Submit');
              await p.space(50);
              await p.signatureField('sig', const PdfRect(x: 72, y: 320, width: 200, height: 60));
              await p.space(40);

              // JavaScript actions on fields
              await p.fieldKeystroke('AFNumber_Keystroke(2, 0, 0, 0, "", true)');
              await p.fieldFormat('AFNumber_Format(2, 0, 0, 0, "", true)');
              await p.fieldValidate('AFRange_Validate(true, 0, true, 100)');
              await p.fieldCalculate('AFSimple_Calculate("SUM", new Array("field1", "field2"))');

              await p.done();

              final sink = MemorySink(); await b.save(sink); await b.dispose();
              final r = sink.takeBytes();
              await saveBytes(r, 'built_form.pdf');
              setState(() => _status = 'Saved (${fmtSize(r.length)})');
            })),

          _Section('Links & References'),
          _Op(icon: Icons.link, title: 'URL link + page link + footnote', loading: _loading,
            onRun: () => _run('Links', () async {
              final b = await _pdf.build();
              await b.setTitle('Links Demo');

              final p1 = await b.addA4Page();
              await p1.heading(1, 'Links & References');
              await p1.space(10);
              await p1.text('Visit:');
              await p1.linkUrl('https://pub.dev/packages/pdf_manipulator');
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

              final sink = MemorySink(); await b.save(sink); await b.dispose();
              final r = sink.takeBytes();
              await saveBytes(r, 'built_links.pdf');
              setState(() => _status = 'Saved (${fmtSize(r.length)})');
            })),

          _Section('Watermark & Image'),
          _Op(icon: Icons.water_drop, title: 'Page with watermark', loading: _loading,
            onRun: () => _run('Watermark', () async {
              final b = await _pdf.build();
              final p = await b.addA4Page();
              await p.heading(1, 'Confidential Report');
              await p.paragraph('Sensitive information.');
              await p.watermark('CONFIDENTIAL');
              await p.done();
              final sink = MemorySink(); await b.save(sink); await b.dispose();
              final r = sink.takeBytes();
              await saveBytes(r, 'built_watermark.pdf');
              setState(() => _status = 'Saved (${fmtSize(r.length)})');
            })),
          _Op(icon: Icons.image, title: 'Page with image (pick)', loading: _loading,
            onRun: () => _run('Image', () async {
              final imgs = await pickImageBytes();
              if (imgs == null || imgs.isEmpty) return;
              final b = await _pdf.build();
              final p = await b.addA4Page();
              await p.heading(1, 'Image Demo');
              await p.image(MemorySource(imgs.first.bytes), const PdfRect(x: 72, y: 500, width: 200, height: 200));
              await p.done();
              final sink = MemorySink(); await b.save(sink); await b.dispose();
              final r = sink.takeBytes();
              await saveBytes(r, 'built_image.pdf');
              setState(() => _status = 'Saved (${fmtSize(r.length)})');
            })),

          _Section('Multi-Page'),
          _Op(icon: Icons.library_books, title: 'newline + newPageSameSize', loading: _loading,
            onRun: () => _run('MultiPage', () async {
              final b = await _pdf.build();
              final p = await b.addA4Page();
              await p.heading(1, 'Page 1');
              await p.paragraph('Some content on page 1.');
              await p.newline();
              await p.text('After a newline.');
              await p.newPageSameSize();
              await p.heading(1, 'Page 2 (same size)');
              await p.paragraph('Created via newPageSameSize().');
              await p.done();
              final sink = MemorySink(); await b.save(sink); await b.dispose();
              final r = sink.takeBytes();
              await saveBytes(r, 'built_multi.pdf');
              setState(() => _status = 'Saved (${fmtSize(r.length)})');
            })),

          _Section('Font'),
          _Op(icon: Icons.font_download, title: 'Custom font + size', loading: _loading,
            onRun: () => _run('Font', () async {
              final b = await _pdf.build();
              final p = await b.addA4Page();
              await p.font('Helvetica', 24);
              await p.text('Large Helvetica');
              await p.font('Courier', 12);
              await p.text('Small Courier');
              await p.font('Times-Roman', 18);
              await p.text('Medium Times');
              await p.done();
              final sink = MemorySink(); await b.save(sink); await b.dispose();
              final r = sink.takeBytes();
              await saveBytes(r, 'built_fonts.pdf');
              setState(() => _status = 'Saved (${fmtSize(r.length)})');
            })),

          const SizedBox(height: 40),
        ])),
      ]),
    );
  }
}

// ─── Tab 6: Merge ─────────────────────────────────────────────────

class _MergeTab extends StatefulWidget {
  const _MergeTab();
  @override
  State<_MergeTab> createState() => _MergeTabState();
}

class _MergeTabState extends State<_MergeTab> {
  final _pdf = Pdf();
  final _files = <({String name, Uint8List bytes, int size})>[];
  Uint8List? _merged;
  bool _loading = false;
  String? _status;

  @override
  void dispose() { _pdf.dispose(); super.dispose(); }

  Future<void> _add() async {
    final result = await FilePicker.platform.pickFiles(allowMultiple: true, type: FileType.custom, allowedExtensions: ['pdf'], withData: true);
    if (result == null) return;
    for (final f in result.files) {
      if (f.bytes != null) _files.add((name: f.name, bytes: f.bytes!, size: f.size));
    }
    setState(() { _merged = null; _status = '${_files.length} files'; });
  }

  Future<void> _merge() async {
    if (_files.length < 2) return;
    setState(() { _loading = true; _status = 'Merging...'; });
    try {
      final sink = MemorySink();
      await _pdf.merge(_files.map((f) => MemorySource(f.bytes) as DataSource).toList(), sink);
      final r = sink.takeBytes();
      setState(() { _merged = r; _status = 'Merged: ${fmtSize(r.length)}'; });
    } catch (e) { setState(() => _status = 'Error: $e'); }
    finally { setState(() => _loading = false); }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Merge PDFs')),
      body: Column(children: [
        _Status(loading: _loading, message: _status, onDismiss: () => setState(() => _status = null)),
        Expanded(child: _files.isEmpty
          ? Center(child: Column(mainAxisSize: MainAxisSize.min, children: [
              Icon(Icons.merge, size: 56, color: Theme.of(context).colorScheme.primary.withValues(alpha: 0.5)),
              const SizedBox(height: 16), const Text('Pick 2+ PDFs to merge'),
              const SizedBox(height: 16), FilledButton.icon(onPressed: _add, icon: const Icon(Icons.add), label: const Text('Pick PDFs')),
            ]))
          : ReorderableListView.builder(
              padding: const EdgeInsets.all(16), itemCount: _files.length,
              onReorder: (old, to) => setState(() { final item = _files.removeAt(old); _files.insert(to, item); _merged = null; }),
              itemBuilder: (_, i) => Card(
                key: ValueKey('${_files[i].name}_$i'),
                margin: const EdgeInsets.symmetric(vertical: 3),
                child: ListTile(
                  leading: CircleAvatar(radius: 16, child: Text('${i + 1}', style: const TextStyle(fontSize: 13))),
                  title: Text(_files[i].name, style: const TextStyle(fontSize: 14)),
                  subtitle: Text(fmtSize(_files[i].size)),
                  trailing: IconButton(icon: const Icon(Icons.close, size: 18), onPressed: () => setState(() { _files.removeAt(i); _merged = null; })),
                  dense: true,
                ),
              ),
            ),
        ),
        if (_files.isNotEmpty) Padding(padding: const EdgeInsets.all(16), child: Row(children: [
          OutlinedButton.icon(onPressed: _loading ? null : _add, icon: const Icon(Icons.add, size: 18), label: const Text('Add')),
          const SizedBox(width: 8),
          Expanded(child: _merged != null
            ? FilledButton.icon(onPressed: () async {
                final p = await saveBytes(_merged!, 'merged.pdf');
                if (p != null) setState(() => _status = 'Saved');
              }, icon: const Icon(Icons.save, size: 18), label: const Text('Save'))
            : FilledButton.icon(
                onPressed: _files.length >= 2 && !_loading ? _merge : null,
                icon: _loading ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white)) : const Icon(Icons.merge, size: 18),
                label: Text('Merge ${_files.length}'))),
        ])),
      ]),
    );
  }
}
